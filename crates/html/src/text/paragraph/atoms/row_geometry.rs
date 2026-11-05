//! Где ряд-обёртка атома ставит его базовую линию (CSS 2.1 §10.8.1,
//! css-align-3 §9.1): базовая читается из готовой раскладки самого атома
//! (`Window::row_item_baseline`), без щупа-соседа и второго прохода.

use gpui::Window;
/// Height, baseline and downward shift of an atom inside its baseline row, as the row aligns
/// it (CSS 2.1 §10.8.1, css-align-3 §9.1): the baseline comes from the item's own layout output
/// (`Window::row_item_baseline`). With `b` the item baseline from the row top, the row puts its
/// baseline at `max(b, 0)`: a negative baseline (a large negative top margin) moves the item
/// down by `-b` and the row grows to hold the margin box below the baseline. Without a
/// participating item the baseline is 0, which `lay_atoms` reads as "no baseline: the bottom
/// of the box". Without a row node the atom has `fallback` height and its bottom as baseline.
pub(crate) fn row_geometry(
    root: Option<gpui::LayoutId>,
    fallback: f32,
    window: &Window,
) -> (f32, f32, f32) {
    let Some(root) = root else {
        return (fallback, fallback, 0.0);
    };
    match window.row_item_baseline(root) {
        Some(gpui::RowItemBaseline {
            baseline: Some(b),
            outer_height,
        }) => from_item_baseline(b, outer_height),
        _ => (fallback, 0.0, 0.0),
    }
}

/// Row height, baseline and item shift for an item with baseline `b` and margin-box height
/// `outer` (see `row_geometry`).
fn from_item_baseline(b: f32, outer: f32) -> (f32, f32, f32) {
    let base = b.max(0.0);
    (base + (outer - b).max(0.0), base, base - b)
}

#[cfg(test)]
mod tests {
    use super::from_item_baseline;

    #[test]
    fn baseline_inside_the_box_keeps_the_margin_box_height_and_no_shift() {
        assert_eq!(from_item_baseline(6.0, 8.0), (8.0, 6.0, 0.0));
    }

    #[test]
    fn bottom_margin_edge_baseline_is_the_full_height() {
        assert_eq!(from_item_baseline(8.0, 8.0), (8.0, 8.0, 0.0));
    }

    #[test]
    fn baseline_below_the_box_grows_the_row_to_the_baseline() {
        assert_eq!(from_item_baseline(12.0, 8.0), (12.0, 12.0, 0.0));
    }

    #[test]
    fn negative_baseline_moves_the_item_down_onto_the_row_baseline() {
        // Row baseline at 0, item shifted down by 5, row holds 8 + 5 below it.
        assert_eq!(from_item_baseline(-5.0, 8.0), (13.0, 0.0, 5.0));
    }
}
