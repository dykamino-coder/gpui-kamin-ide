//! Исходный paint-слот позиционированного элемента, построенного у содержащего блока.

use crate::hooks::{IntoHooked, PaintOnly, Painter};
use crate::layout::positioned::containing_block::DeferredPaint;
use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, Window};

pub(crate) fn source(publication: DeferredPaint) -> AnyElement {
    PaintOnly(DeferredSource(publication)).hooked()
}

struct DeferredSource(DeferredPaint);

impl Painter for DeferredSource {
    type Prepaint = ();

    fn style(&self) -> gpui::Style {
        gpui::Style {
            position: gpui::Position::Absolute,
            ..Default::default()
        }
    }

    fn paint(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut LayoutId,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.0.with_child(|child, mask| {
            // Исходная маска дополняет clip фрагмента, установленный paint.
            window.with_content_mask(Some(mask), |window| {
                gpui::paint_reopen(|| child.paint(window, cx));
            });
        });
    }
}
