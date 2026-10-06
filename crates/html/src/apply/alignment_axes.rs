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

/// CSS Align 3 positional-values: self-relative edges follow the subject;
/// left/right follow line-left/line-right rather than the container's direction.
pub(super) fn grid_self(style: &mut gpui::StyleRefinement, c: &crate::computed::Computed) {
    if c.parent_grid == 0 {
        return;
    }
    let vertical = c.parent_grid >= 2;
    let parent_x_end = c.parent_grid == 3 || (!vertical && c.cb_rtl);
    let own = super::grid_flow_axes::reversed(c);
    let (align_axis, justify_axis) = if vertical { (0, 1) } else { (1, 0) };
    let mirror = |value: &mut Option<gpui::AlignSelf>, flip: bool| {
        if !flip {
            return;
        }
        *value = value.map(|value| match value {
            gpui::AlignSelf::Start => gpui::AlignSelf::End,
            gpui::AlignSelf::End => gpui::AlignSelf::Start,
            gpui::AlignSelf::FlexStart => gpui::AlignSelf::FlexEnd,
            gpui::AlignSelf::FlexEnd => gpui::AlignSelf::FlexStart,
            other => other,
        });
    };
    if c.align_self_own_axis {
        mirror(
            &mut style.align_self,
            own[align_axis] != (align_axis == 0 && parent_x_end),
        );
    }
    if c.justify_self_own_axis {
        mirror(
            &mut style.justify_self,
            own[justify_axis] != (justify_axis == 0 && parent_x_end),
        );
    }
    if let Some(right) = c.justify_self_physical {
        let end = if vertical {
            right != (c.cb_sideways && !c.cb_vertical_rl)
        } else {
            right != parent_x_end
        };
        style.justify_self = Some(if end {
            gpui::AlignSelf::FlexEnd
        } else {
            gpui::AlignSelf::FlexStart
        });
    }
}
