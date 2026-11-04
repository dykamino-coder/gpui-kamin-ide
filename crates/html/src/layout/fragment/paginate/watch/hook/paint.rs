//! Краска наблюдаемого узла с продолжением на общем стеке.
use super::*;

impl Watch {
    pub(super) async fn paint_children(
        &mut self,
        prepaint: &mut Option<ContentMask<Pixels>>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) {
        let access = match self.slot {
            Slot::Box(id) => self.rec.access(id, window.content_mask()),
            Slot::Table(..) => super::super::owner::Access::Normal,
        };
        if let Slot::Box(id) = self.slot
            && let Some(slices) = self.rec.admitted(id)
        {
            let inner = &mut self.inner;
            self.rec
                .with_admitted(slices.clone(), async || {
                    admitted_paint(inner, *prepaint, slices, window, cx, stack).await;
                })
                .await;
            return;
        }
        match access {
            super::super::owner::Access::Hidden => {}
            super::super::owner::Access::Escape(mask) => {
                window
                    .with_content_mask_replaced_async(mask, async |window| {
                        window
                            .with_content_mask_async(*prepaint, async |window| {
                                self.inner.paint_async(window, cx, stack).await;
                            })
                            .await;
                    })
                    .await
            }
            super::super::owner::Access::Normal => {
                window
                    .with_content_mask_async(*prepaint, async |window| {
                        self.inner.paint_async(window, cx, stack).await;
                    })
                    .await
            }
        }
    }
}

async fn admitted_paint(
    inner: &mut AnyElement,
    mask: Option<ContentMask<Pixels>>,
    slices: Rc<[gpui::PaintSegmentSlice]>,
    window: &mut Window,
    cx: &mut App,
    stack: &mut gpui::PaintStack,
) {
    let _ = window
        .with_paint_segment_slices_async(slices, async |window| {
            window
                .with_content_mask_async(mask, async |window| {
                    inner.paint_async(window, cx, stack).await;
                })
                .await;
        })
        .await;
}
