//! Рост независимого потока flex по текущему плану фрагментаторов (css-flexbox-1 §10).
use super::Growth;
use crate::layout::fragment::flow::FlowGeometry;
use crate::layout::fragment::flow::blocks::{BlockBox, Laid, Pushed};
use crate::tolerance::COORD_EPS;
mod items;
use items::{column_item, pushed_past_edge, row_item_line_start, row_item_top};

/// Точка роста относится к одному потоку: соседний flex-item с той же координатой
/// не должен получить его распорку или принудительный разрыв. `open` — фрагмент
/// начинается с верха колонки ряда не с начала внешнего фрагментаинера: неделимое и
/// пожелания с его верха переносятся туда, где места больше (Blink `LayoutRow`).
pub(crate) fn growths(
    blocks: &BlockBox,
    laid: &Laid,
    from: f32,
    end: f32,
    edge: f32,
    open: bool,
) -> Vec<Growth> {
    let mut flows = Vec::new();
    blocks.flex_flows(laid, &mut flows);
    let mut out = Vec::new();
    let mut boundaries = Vec::new();
    blocks.zero_forced_flex_lines(laid, &mut boundaries);
    for (target, at) in boundaries {
        if at >= from - COORD_EPS && at <= end + COORD_EPS && at < edge - COORD_EPS {
            out.push(Growth {
                target: crate::layout::fragment::flow::blocks::Pushed::Flex(target),
                at,
                amount: edge - at,
                forced: true,
            });
        }
    }
    let mut gaps = Vec::new();
    blocks.suppressed_row_gaps(laid, from, end, edge, &mut gaps);
    flow_growths(&flows, laid, from, end, edge, open, &mut out);
    // Линия, чей элемент сам растёт в этом фрагменте (принудительный разрыв
    // внутри, перенос монолита), ещё не кончилась: промежуток после неё лежит
    // за её продолжением, а не у этого края (css-flexbox-1 §10).
    for (target, start, next) in gaps {
        if !out
            .iter()
            .any(|g| g.amount > 0.0 && g.at > start - COORD_EPS && g.at < next - COORD_EPS)
        {
            out.push(Growth {
                target: Pushed::Flex(target),
                at: next,
                amount: edge - next,
                forced: false,
            });
        }
    }
    let mut starts = Vec::new();
    blocks.forced_line_starts(laid, &mut starts);
    for (id, start) in starts {
        if crate::layout::fragment::flow::blocks::forced_line_start_pending(start, from, edge) {
            out.push(Growth {
                target: Pushed::Box(id),
                at: start,
                amount: edge - start,
                forced: true,
            });
        }
    }
    out
}

/// Рост каждого независимого потока flex: колонки и содержимое элементов.
fn flow_growths(
    flows: &[BlockBox],
    laid: &Laid,
    from: f32,
    end: f32,
    edge: f32,
    open: bool,
    out: &mut Vec<Growth>,
) {
    let after_start = |a: f32| a > from + COORD_EPS || (open && a > from - COORD_EPS);
    for flow in flows {
        // В открытом ряду перенос за край — в следующий ряд, не к этому краю.
        if let Some(growth) = pushed_past_edge(&flow, laid, from, edge).filter(|_| !open) {
            out.push(growth);
            continue;
        }
        let breaks = flow
            .breaks(f32::MAX, laid)
            .finish(FlowGeometry::default(), f32::MAX);
        // Внутри выделенного места элемента колонки разрывы обслуживает
        // последовательный поток. За его концом содержимое независимо от
        // следующего элемента (css-break-3 §2.1); цель ищется в самом item.
        let overflow_start = column_item(laid, flow.id)
            .then(|| laid.at(flow.id).map(|p| p.1))
            .flatten();
        let local = |at: f32| overflow_start.is_none_or(|start| at >= start - COORD_EPS);
        let overflow_edge = overflow_start.is_none_or(|start| edge > start + COORD_EPS);
        let forced = breaks
            .forced
            .iter()
            .copied()
            .filter(|&f| {
                local(f) && f > from + COORD_EPS && f < edge - COORD_EPS && f <= end + COORD_EPS
            })
            .min_by(f32::total_cmp);
        // Мягкий avoid закрывает и точную границу соседей без монолита.
        // Внутри пустого элемента сам диапазон не запрещает разрез класса C.
        let boundary = breaks.cuts.iter().any(|p| (p.0 - edge).abs() < COORD_EPS);
        // Пожелание с начала элемента сетки выполнил бы только перенос его ряда: ряд
        // переносит лишь первая неделимая часть и `break-inside: avoid`
        // (`GridBox::breaks`), остальное ослабляется внутри элемента.
        let grid_top = laid
            .placements
            .contains_key(&flow.id)
            .then(|| laid.at(flow.id).map(|p| p.0))
            .flatten();
        let wish = |p: &&(f32, f32)| grid_top.is_none_or(|top| p.0 > top + COORD_EPS);
        let keep: Vec<_> = breaks.keep.iter().filter(wish).copied().collect();
        let avoid: Vec<_> = breaks.avoid.iter().filter(wish).copied().collect();
        let solid = breaks
            .solid
            .iter()
            .chain(&keep)
            .chain(avoid.iter().filter(|_| boundary))
            .filter(|&&(a, b)| {
                overflow_edge
                    && after_start(a)
                    && a <= end + COORD_EPS
                    && a < edge - COORD_EPS
                    && b > edge + COORD_EPS
            })
            .map(|p| p.0)
            .min_by(f32::total_cmp);
        let solid = solid.map(|mut point| {
            for _ in 0..keep.len() + avoid.len() {
                let start = keep
                    .iter()
                    .chain(&avoid)
                    .filter(|&&(a, b)| {
                        after_start(a) && a < point - COORD_EPS && point < b - COORD_EPS
                    })
                    .map(|p| p.0)
                    .min_by(f32::total_cmp);
                let Some(start) = start else { break };
                point = start;
            }
            point
        });
        let point = match (forced, solid) {
            (Some(f), Some(s)) if s < f => Some((s, false)),
            (Some(f), _) => Some((f, true)),
            (_, Some(s)) => Some((s, false)),
            _ => None,
        };
        let Some((point, forced)) = point else {
            continue;
        };
        // Первая неделимая часть элемента ряда переносит всю линию
        // (`FlexBox::breaks`): её рост — рост линии, а не потока элемента.
        if let Some(top) = row_item_top(laid, flow.id)
            && point <= top + COORD_EPS
        {
            // Элемент, выровненный ниже начала линии, переносится сам.
            if let Some(start) = row_item_line_start(laid, flow.id)
                && laid.margin_box(flow.id).map_or(top, |m| m.0) > start + COORD_EPS
                && edge > point + COORD_EPS
            {
                out.push(Growth {
                    target: Pushed::Box(flow.id),
                    at: point,
                    amount: edge - point,
                    forced,
                });
            }
            continue;
        }
        let at = breaks
            .cuts
            .iter()
            .find(|p| (p.0 - point).abs() < COORD_EPS)
            .map_or(point, |p| p.1);
        let Some(mut growth) = Growth::at(&flow, laid, at, edge - point, forced) else {
            continue;
        };
        // Элемент колонки после разрыва начинается с края фрагментаинера:
        // промежуток (gap) у разрыва убирается целиком (css-gaps-1 §2;
        // Blink `UpdateOffsetAdjustmentForSuppressedRowGap`), поле после
        // принудительного разрыва остаётся при элементе (css-break-3 §5.2).
        if let Pushed::Box(id) = growth.target
            && column_item(laid, id)
        {
            growth.amount = edge - at;
        }
        if growth.amount > COORD_EPS {
            out.push(growth);
        }
    }
}
