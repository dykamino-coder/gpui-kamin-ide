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

    fn paint_async<'a>(
        &'a mut self,
        _: Bounds<Pixels>,
        _: &'a mut LayoutId,
        _: &'a mut (),
        window: &'a mut Window,
        cx: &'a mut App,
        stack: &'a mut gpui::PaintStack,
    ) -> impl std::future::Future<Output = ()> + 'a {
        async move {
            self.0
                .with_child(async |child, mask| {
                    // Исходная маска дополняет clip фрагмента, установленный paint.
                    window
                        .with_content_mask_async(Some(mask), async |window| {
                            crate::hooks::paint_reopen_async(window, async |window| {
                                child.paint_async(window, cx, stack).await;
                            })
                            .await;
                        })
                        .await;
                })
                .await;
        }
    }
}
