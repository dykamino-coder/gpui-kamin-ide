//! Условия переноса атомов по вертикальной строчной оси.
use crate::dom::Node;
use crate::style::computed::{BoxStyle, Computed};
use crate::style::values::value::Len;

/// Several atoms share a vertical line and wrap along the inline (physical vertical)
/// axis when its length is known: the box continues its parent's writing mode, its
/// inline size is authored, or a single-line flex/grid container decides it (the
/// turned line then wraps at the length prepaint lays it out with).
pub(super) fn turned_atom_line(nodes: &[Node], style: &BoxStyle) -> bool {
    let atoms = nodes
        .iter()
        .filter(|n| matches!(n, Node::Element(_)))
        .count();
    let wm = style.context.cb_wm;
    let parallel = !style.wm().is_orthogonal_to(wm);
    atoms >= 2
        && (parallel
            || style_definite_inline(style)
            || ((style.reset_flex.flex_item || style.context.parent_flex_grid)
                && !style.reset_flex.flex_line_wraps))
}

pub(super) fn style_definite_inline(style: &Computed) -> bool {
    matches!(
        style.reset_box_layout.height,
        Some(Len::Px(_)) | Some(Len::Pct(_))
    )
}
