//! Экстенты атома в IFC (CSS 2.1 §10.8.1, css-align-3 §9.1):
//! коробка полей и базовая читаются из готовой раскладки самого атома.

use gpui::Window;
/// IFC ascent/descent and placement shift from the atom's completed output.
/// A baseline above its margin box is normalized to zero for the line model;
/// `drop` retains the matching placement displacement. An absent participating
/// baseline uses the margin-box bottom in `lay_atoms`.
pub(crate) fn row_geometry(
    root: Option<gpui::LayoutId>,
    fallback: f32,
    window: &Window,
) -> (f32, f32, f32) {
    let Some(root) = root else {
        return (fallback, fallback, 0.0);
    };
    match window.inline_geometry(root).1 {
        gpui::RowItemBaseline {
            baseline: Some(b),
            outer_height,
        } => from_item_baseline(b, outer_height),
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
