//! Проекция prepaint, явно разделяемая элементами одного дерева render.

use gpui::{Bounds, Pixels, point};
use std::cell::Cell;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TurnFrame {
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) ccw: bool,
    pub(crate) phase: Pixels,
}

#[derive(Default)]
pub(crate) struct VerticalPaint(Cell<Option<TurnFrame>>);

impl VerticalPaint {
    pub(crate) fn frame(&self) -> Option<TurnFrame> {
        self.0.get()
    }

    pub(crate) fn replace(&self, frame: Option<TurnFrame>) -> Option<TurnFrame> {
        self.0.replace(frame)
    }

    pub(crate) fn is_rotated(&self) -> bool {
        self.frame().is_some()
    }

    /// Map a flat static-position hole into the active text frame's screen plane.
    pub(crate) fn map(&self, hole: Bounds<Pixels>, thickness: Pixels) -> Bounds<Pixels> {
        let Some(frame) = self.frame() else {
            return hole;
        };
        let vt = frame.bounds;
        let pre_x = hole.origin.x - vt.origin.x;
        let pre_y = hole.origin.y - vt.origin.y;
        let origin = if frame.ccw {
            point(
                vt.origin.x + pre_y,
                vt.origin.y + vt.size.height - pre_x - thickness,
            )
        } else {
            point(
                vt.origin.x + vt.size.width - (pre_y - frame.phase) - thickness,
                vt.origin.y + pre_x,
            )
        };
        Bounds {
            origin,
            size: hole.size,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{px, size};

    fn frame(ccw: bool) -> TurnFrame {
        TurnFrame {
            bounds: Bounds {
                origin: point(px(10.), px(20.)),
                size: size(px(80.), px(100.)),
            },
            ccw,
            phase: px(0.25),
        }
    }

    #[test]
    fn nested_and_physical_subtrees_restore_only_their_own_render_context() {
        let a = VerticalPaint::default();
        let b = VerticalPaint::default();
        let outer = a.replace(Some(frame(false)));
        assert_eq!(outer, None);
        assert!(!b.is_rotated());
        let inner = a.replace(Some(frame(true)));
        let physical = a.replace(None);
        assert!(!a.is_rotated());
        a.replace(physical);
        assert_eq!(a.frame(), Some(frame(true)));
        a.replace(inner);
        assert_eq!(a.frame(), Some(frame(false)));
        a.replace(outer);
        assert!(!a.is_rotated());
        assert!(!b.is_rotated());
    }

    #[test]
    fn static_holes_follow_direction_and_phase_without_mutating_other_trees() {
        let a = VerticalPaint::default();
        let b = VerticalPaint::default();
        let hole = Bounds {
            origin: point(px(15.), px(27.)),
            size: size(px(2.), px(3.)),
        };
        a.replace(Some(frame(false)));
        assert_eq!(a.map(hole, px(4.)).origin, point(px(79.25), px(25.)));
        assert_eq!(b.map(hole, px(4.)), hole);
        a.replace(Some(frame(true)));
        assert_eq!(a.map(hole, px(4.)).origin, point(px(17.), px(111.)));
    }
}
