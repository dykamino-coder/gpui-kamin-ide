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

/// Верх элемента ряда flex, чей поток — `id`.
fn row_item_top(laid: &Laid, id: u64) -> Option<f32> {
    laid.flexes
        .values()
        .any(|f| !f.column && f.lines.iter().any(|l| l.items.contains(&id)))
        .then(|| laid.at(id).map(|p| p.0))
        .flatten()
}

/// Элемент колонки flex: сдвигается своим смещением, а не распоркой.
fn column_item(laid: &Laid, id: u64) -> bool {
    laid.flexes
        .values()
        .any(|f| f.column && f.lines.iter().any(|l| l.items.contains(&id)))
}
