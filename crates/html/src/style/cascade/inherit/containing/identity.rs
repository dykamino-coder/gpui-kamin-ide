//! Идентичность ближайших CB: absolute и fixed имеют разные цепочки владельцев.

use crate::style::computed::{BoxStyle, stacking::fixed_cb_layer_box};

pub(super) fn inherit(parent: &BoxStyle, child: &mut BoxStyle) {
    child.cb_node = if super::establishes_cb(parent) {
        parent.self_node
    } else {
        parent.cb_node
    };
    child.fixed_cb_node = if fixed_cb_layer_box(parent) {
        parent.self_node
    } else {
        parent.fixed_cb_node
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::computed::Position;

    #[test]
    fn relative_cb_does_not_replace_nearest_fixed_cb() {
        let mut outer = BoxStyle::default();
        outer.self_node = 10;
        outer.contain_layout = Some(true);
        let mut relative = BoxStyle::default();
        relative.self_node = 20;
        relative.position = Some(Position::Relative);
        inherit(&outer, &mut relative);
        let mut fixed = BoxStyle::default();
        inherit(&relative, &mut fixed);
        assert_eq!(fixed.cb_node, 20);
        assert_eq!(fixed.fixed_cb_node, 10);
    }

    #[test]
    fn nearest_fixed_cb_replaces_outer_and_viewport_has_zero_identity() {
        let mut outer = BoxStyle::default();
        outer.self_node = 10;
        outer.contain_paint = Some(true);
        let mut inner = BoxStyle::default();
        inner.self_node = 20;
        inner.will_change = crate::style::computed::wc::CB_FIXED;
        inherit(&outer, &mut inner);
        let mut child = BoxStyle::default();
        inherit(&inner, &mut child);
        assert_eq!(child.fixed_cb_node, 20);
        inherit(&BoxStyle::default(), &mut child);
        assert_eq!(child.fixed_cb_node, 0);
    }
}
