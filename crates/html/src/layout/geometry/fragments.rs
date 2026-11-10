//! Физическая блочная ось и точный срез существуют лишь во время явного запроса.

use gpui::{Bounds, Pixels, Window};
use std::rc::Rc;

pub(crate) struct BlockAxis(pub(crate) bool);
impl BlockAxis {
    pub(crate) fn vertical(window: &Window) -> bool {
        window.render_context::<Self>().is_some_and(|axis| axis.0)
    }
    pub(crate) fn admits(window: &Window, b: Bounds<Pixels>, clip: Bounds<Pixels>) -> bool {
        if Self::vertical(window) {
            if b.size.width == gpui::px(0.) {
                b.left() >= clip.left() && b.left() <= clip.right()
            } else {
                b.right() > clip.left() && b.left() < clip.right()
            }
        } else if b.size.height == gpui::px(0.) {
            b.top() >= clip.top() && b.top() <= clip.bottom()
        } else {
            b.bottom() > clip.top() && b.top() < clip.bottom()
        }
    }

    pub(crate) fn empty(window: &Window, b: Bounds<Pixels>) -> bool {
        if Self::vertical(window) {
            b.size.width == gpui::px(0.)
        } else {
            b.size.height == gpui::px(0.)
        }
    }
    pub(crate) fn slice(
        window: &Window,
        mut b: Bounds<Pixels>,
        clip: Bounds<Pixels>,
    ) -> Bounds<Pixels> {
        if Self::vertical(window) {
            let right = b.right().min(clip.right()).max(clip.left());
            b.origin.x = b.left().max(clip.left()).min(clip.right());
            b.size.width = (right - b.left()).max(gpui::px(0.));
        } else {
            let bottom = b.bottom().min(clip.bottom()).max(clip.top());
            b.origin.y = b.top().max(clip.top()).min(clip.bottom());
            b.size.height = (bottom - b.top()).max(gpui::px(0.));
        }
        b
    }
}

pub(crate) fn with_slice<R>(
    window: &mut Window,
    clip: Bounds<Pixels>,
    f: impl FnOnce(&mut Window) -> R,
) -> R {
    let origin = clip.origin;
    let slice = gpui::PaintSegmentSlice {
        placement: gpui::PaintSegment {
            id: gpui::PaintSegmentId(u64::MAX),
            ordinal: 0,
            source: origin,
            destination: origin,
        },
        source_clip: clip,
    };
    window.with_layout_fragment_slices(Rc::from([slice]), f)
}

/// A query replaces the source's ancestor chain while retaining local CSS transforms.
pub(crate) fn with_hit_scopes<const QUERY: bool, R>(
    scopes: Rc<[(gpui::PaintSegmentId, u32, gpui::PaintHitScope)]>,
    window: &mut Window,
    f: impl FnOnce(&mut Window) -> R,
) -> R {
    if QUERY {
        window.with_layout_fragment_hit_scopes(scopes, f)
    } else {
        window.with_prepaint_segment_hit_scopes(scopes, f)
    }
}
