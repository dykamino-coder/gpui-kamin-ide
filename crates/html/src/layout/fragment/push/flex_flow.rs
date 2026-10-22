//! Рост независимого потока flex по текущему плану фрагментаторов (css-flexbox-1 §10).
use crate::layout::fragment::flow::FlowGeometry;
use crate::layout::fragment::flow::blocks::{BlockBox, Laid};
use crate::tolerance::COORD_EPS;

/// Точка роста относится к одному потоку: соседний flex-item с той же координатой
/// не должен получить его распорку или принудительный разрыв.
pub(crate) fn growths(
    blocks: &BlockBox,
    laid: &Laid,
    from: f32,
    end: f32,
    edge: f32,
) -> Vec<(BlockBox, f32, f32, bool)> {
    let mut flows = Vec::new();
    blocks.flex_flows(laid, &mut flows);
    let mut out = Vec::new();
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
        let at = breaks
            .cuts
            .iter()
            .find(|p| (p.0 - point).abs() < COORD_EPS)
            .map_or(point, |p| p.1);
        out.push((flow, at, edge - point, forced));
    }
    out
}
