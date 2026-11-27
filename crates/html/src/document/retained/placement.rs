//! Last observed embedding inputs, owned by the window's document element state.
//! Layout preparation re-enters these inputs; prepaint still validates the live key.

use gpui::{Bounds, ContentMask, ElementLayoutContext, Pixels, Point, Window, px, size};

pub(super) struct Placement {
    pub(super) bounds: Bounds<Pixels>,
    viewport: (f32, f32),
    context: ElementLayoutContext,
    mask: ContentMask<Pixels>,
    offset: Point<Pixels>,
}

impl Placement {
    pub(super) fn capture(bounds: Bounds<Pixels>, viewport: (f32, f32), window: &Window) -> Self {
        Self {
            bounds,
            viewport,
            context: window.element_layout_context(),
            mask: window.content_mask(),
            offset: window.element_offset(),
        }
    }

    /// Re-enter the host's resolved box while its authored viewport is unchanged.
    /// At fractional DPI, the pixel-snapped box can differ from that viewport.
    /// A real viewport change must use new constraints and be checked in prepaint.
    pub(super) fn bounds_for(&self, viewport: (f32, f32)) -> Bounds<Pixels> {
        if self.viewport == viewport {
            self.bounds
        } else {
            Bounds::new(self.bounds.origin, size(px(viewport.0), px(viewport.1)))
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
