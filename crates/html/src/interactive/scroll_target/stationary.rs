//! Неподвижная обёртка внутри scrollport: компенсирует смещение прокрутки и красит ребёнка в
//! обрезке декораций, потому что рамки scrollport не принадлежат его прокручиваемому
//! содержимому (CSS Overflow 3 §2).

use crate::hooks::{IntoHooked, Wrap};
use gpui::{AnyElement, App, Bounds, ContentMask, Pixels, ScrollHandle, Window};
use std::{cell::Cell, rc::Rc};

pub(super) fn wrap(
    child: AnyElement,
    handle: ScrollHandle,
    clip: Rc<Cell<Option<ContentMask<Pixels>>>>,
) -> AnyElement {
    Stationary {
        child,
        handle,
        clip,
    }
    .hooked()
}
struct Stationary {
    child: AnyElement,
    handle: ScrollHandle,
    clip: Rc<Cell<Option<ContentMask<Pixels>>>>,
}
impl Wrap for Stationary {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }
    async fn prepaint_async(
        &mut self,
        _: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) {
        let clip = self.clip.get().unwrap_or_else(|| window.content_mask());
        window
            .with_content_mask_replaced_async(clip, async |window| {
                window
                    .with_exact_element_offset_async(-self.handle.offset(), async |window| {
                        self.child.prepaint_async(window, cx, stack).await;
                    })
                    .await;
            })
            .await;
    }
    async fn paint_async(
        &mut self,
        _: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) {
        let clip = self.clip.get().unwrap_or_else(|| window.content_mask());
        window
            .with_content_mask_replaced_async(clip, async |window| {
                self.child.paint_async(window, cx, stack).await
            })
            .await;
    }
}
