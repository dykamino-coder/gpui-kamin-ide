//! Project grid alignment values and safety together onto physical Taffy axes.
//! A vertical container exchanges items/content; a vertical parent exchanges
//! the child's self alignment independently of the child's own writing mode.

pub(super) fn abspos_normal(style: &mut gpui::StyleRefinement, c: &crate::computed::Computed) {
    use crate::computed::Position;
    if !matches!(c.position, Some(Position::Absolute | Position::Fixed)) {
        return;
    }
    // CSS Align 3 §align-abspos/justify-abspos: normal is start when
    // determining the static position. Unlike auto, it does not inherit items.
    // Native abspos sizing handles stretching between two definite insets.
    if c.align_self_normal {
        style.align_self = Some(gpui::AlignSelf::Start);
    }
    if c.justify_self_normal {
        style.justify_self = Some(gpui::AlignSelf::Start);
    }
}

pub(super) fn project(style: &mut gpui::StyleRefinement, container: bool, parent: bool) {
    if container {
        std::mem::swap(&mut style.align_items, &mut style.justify_items);
        std::mem::swap(&mut style.align_content, &mut style.justify_content);
    }
    if parent {
        std::mem::swap(&mut style.align_self, &mut style.justify_self);
    }
    if let (Some(align), Some(justify)) = (
        style.safe_alignment.as_mut(),
        style.safe_justify_alignment.as_mut(),
    ) {
        if container {
            std::mem::swap(&mut align.0, &mut justify.0);
            std::mem::swap(&mut align.2, &mut align.3);
        }
        if parent {
            std::mem::swap(&mut align.1, &mut justify.1);
        }
    }
}
