//! Физические срезы normal и OOF имеют одну логическую ось фрагментации.

use crate::layout::fragment::types::{StackAxis, axis_box};
use gpui::{Bounds, Pixels};

pub(super) fn inline(axis: StackAxis, bounds: Bounds<Pixels>) -> f32 {
    if axis.is_vertical() {
        bounds.size.height.into()
    } else {
        bounds.size.width.into()
    }
}

pub(super) fn block(axis: StackAxis, bounds: Bounds<Pixels>) -> f32 {
    if axis.is_vertical() {
        bounds.size.width.into()
    } else {
        bounds.size.height.into()
    }
}

pub(super) fn slice(
    axis: StackAxis,
    bounds: Bounds<Pixels>,
    io: f32,
    ie: f32,
    bo: f32,
    be: f32,
) -> Bounds<Pixels> {
    axis_box(axis, bounds.origin, block(axis, bounds), io, ie, bo, be)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{point, px, size};

    #[test]
    fn continuation_uses_physical_left_edge_for_vertical_rl_translation() {
        let rect = Bounds::new(point(px(10.), px(20.)), size(px(120.), px(50.)));
        let part = slice(StackAxis::VerticalRl, rect, 0., 50., 30., 40.);
        assert_eq!(
            part,
            Bounds::new(point(px(60.), px(20.)), size(px(40.), px(50.)))
        );
        let next = slice(StackAxis::VerticalRl, rect, 0., 50., 70., 40.);
        assert_eq!(next.origin.x, px(20.));
        assert_eq!(
            slice(StackAxis::VerticalLr, rect, 0., 50., 30., 40.)
                .origin
                .x,
            px(40.)
        );
    }
}
