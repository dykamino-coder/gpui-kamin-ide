//! Рост независимого потока flex по текущему плану фрагментаторов (css-flexbox-1 §10).
use super::Growth;
use crate::layout::fragment::flow::FlowGeometry;
use crate::layout::fragment::flow::blocks::{BlockBox, Laid, Pushed};
use crate::tolerance::COORD_EPS;

/// Точка роста относится к одному потоку: соседний flex-item с той же координатой
/// не должен получить его распорку или принудительный разрыв.
pub(crate) fn growths(
    blocks: &BlockBox,
    laid: &Laid,
    from: f32,
    end: f32,
    edge: f32,
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
    for flow in flows {
        if let Some(growth) = pushed_past_edge(&flow, laid, from, edge) {
            out.push(growth);
            continue;
        }
        let breaks = flow
            .breaks(f32::MAX, laid)
            .finish(FlowGeometry::default(), f32::MAX);
        let forced = breaks
            .forced
            .iter()
            .copied()
            .filter(|&f| f > from + COORD_EPS && f < edge - COORD_EPS && f <= end + COORD_EPS)
            .min_by(f32::total_cmp);
        let solid = breaks
            .solid
            .iter()
            .chain(&breaks.keep)
            .filter(|&&(a, b)| {
                a > from + COORD_EPS
                    && a <= end + COORD_EPS
                    && a < edge - COORD_EPS
                    && b > edge + COORD_EPS
            })
            .map(|p| p.0)
            .min_by(f32::total_cmp);
        let solid = solid.map(|mut point| {
            for _ in 0..breaks.keep.len() {
                let start = breaks
                    .keep
                    .iter()
                    .filter(|&&(a, b)| {
                        a > from + COORD_EPS && a < point - COORD_EPS && point < b - COORD_EPS
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
    out
}

/// Элемент колонки, начинающийся за краем фрагментаинера после содержимого
/// этого фрагмента: разрыв перед ним, а промежуток между ними (row-gap и место
/// `justify-content`), не поместившийся до края, гасится — элемент встаёт на
/// начало следующего фрагментаинера (Blink
/// `UpdateOffsetAdjustmentForSuppressedRowGap`, эффективный промежуток
/// `effective_gap_between_items`). Сдвиг отрицательный; заданный размер
/// контейнера он не меняет.
fn pushed_past_edge(flow: &BlockBox, laid: &Laid, from: f32, edge: f32) -> Option<Growth> {
    flow.flex_gap?;
    let mut prev_end: Option<f32> = None;
    for kid in &flow.kids {
        let (top, bottom) = laid.margin_box(kid.id).or_else(|| laid.at(kid.id))?;
        if top >= edge - COORD_EPS {
            let ended = prev_end.is_some_and(|e| e > from + COORD_EPS && e <= edge + COORD_EPS);
            if ended && top > edge + COORD_EPS {
                return Some(Growth {
                    target: Pushed::Box(kid.id),
                    at: top,
                    amount: edge - top,
                    forced: false,
                });
            }
            return None;
        }
        prev_end = Some(bottom);
    }
    None
}

/// Верх элемента ряда flex, чей поток — `id`.
fn row_item_top(laid: &Laid, id: u64) -> Option<f32> {
    laid.flexes
        .values()
        .any(|f| !f.column && f.lines.iter().any(|l| l.items.contains(&id)))
        .then(|| laid.at(id).map(|p| p.0))
        .flatten()
}

/// Начало линии ряда, в которой стоит элемент `id`.
fn row_item_line_start(laid: &Laid, id: u64) -> Option<f32> {
    laid.flexes
        .values()
        .filter(|f| !f.column)
        .flat_map(|f| &f.lines)
        .find(|l| l.items.contains(&id))
        .map(|l| l.start)
}

/// Элемент колонки flex: сдвигается своим смещением, а не распоркой.
fn column_item(laid: &Laid, id: u64) -> bool {
    laid.flexes
        .values()
        .any(|f| f.column && f.lines.iter().any(|l| l.items.contains(&id)))
}
