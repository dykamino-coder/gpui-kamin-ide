//! Last observed embedding inputs, owned by the window's document element state.
//! Layout preparation re-enters these inputs; prepaint still validates the live key.

use gpui::{Bounds, ContentMask, ElementLayoutContext, Pixels, Point, Window};

pub(super) struct Placement {
    pub(super) bounds: Bounds<Pixels>,
    context: ElementLayoutContext,
    mask: ContentMask<Pixels>,
    offset: Point<Pixels>,
}

impl Placement {
    pub(super) fn capture(bounds: Bounds<Pixels>, window: &Window) -> Self {
        Self {
            bounds,
            context: window.element_layout_context(),
            mask: window.content_mask(),
            offset: window.element_offset(),
        }
    }

    pub(super) fn enter<R>(&self, window: &mut Window, f: impl FnOnce(&mut Window) -> R) -> R {
        window.with_element_layout_context(&self.context, |window| {
            window.with_content_mask_exact(self.mask, |window| {
                window.with_absolute_element_offset(self.offset, f)
            })
        })
    }
}
