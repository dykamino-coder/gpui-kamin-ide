//! Разделение позиционированной коробки на две половины: раскладку у содержащего блока и
//! отрисовку на месте в дереве. CSS 2.1 Appendix E красит позиционированные в порядке
//! разметки независимо от содержащего блока, поэтому `SourcePaint` без заместителя
//! запрашивает раскладку у CB, а с заместителем делает `prepaint`/`paint` из исходного места.

use crate::hooks::{Hooks, IntoHooked};
use crate::layout::positioned::containing_block::{LatePlace, SpotCell};
use gpui::{
    AnyElement, App, Bounds, IntoElement, LayoutId, Pixels, Styled, Window, div, point, px,
};
use std::{cell::RefCell, rc::Rc};

/// CSS 2 Appendix E orders painting independently of the containing block.
/// Only the layout half belongs to the CB; the source half owns prepaint/paint.
pub(crate) fn split(spot: SpotCell, child: AnyElement) -> (AnyElement, AnyElement) {
    let child = Rc::new(RefCell::new(PlacedChild {
        element: LatePlace { child, spot }.hooked(),
        layout: None,
    }));
    let layout = SourcePaint {
        child: child.clone(),
        placeholder: None,
    };
    let paint = SourcePaint {
        child,
        placeholder: Some(div().absolute().w_0().h_0().into_any_element()),
    };
    (layout.hooked(), paint.hooked())
}

struct SourcePaint {
    child: Rc<RefCell<PlacedChild>>,
    placeholder: Option<AnyElement>,
}

struct PlacedChild {
    element: AnyElement,
    layout: Option<gpui::RetainedLayout>,
}

impl Hooks for SourcePaint {
    type Layout = ();
    type Prepaint = ();

    #[expect(
        clippy::await_holding_refcell_ref,
        reason = "exclusive slot access spans same-thread PaintStack child polls; no executor yield"
    )]
    async fn request_layout_async(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> (LayoutId, ()) {
        let layout = match self.placeholder.as_mut() {
            Some(placeholder) => placeholder.request_layout_async(window, cx, stack).await,
            None => {
                let mut child = self.child.borrow_mut();
                child.layout = Some(window.retained_layout());
                child.element.request_layout_async(window, cx, stack).await
            }
        };
        (layout, ())
    }

    #[expect(
        clippy::await_holding_refcell_ref,
        reason = "exclusive slot access spans same-thread PaintStack child polls; no executor yield"
    )]
    async fn prepaint_async(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) {
        if self.placeholder.is_some() {
            let mut child = self.child.borrow_mut();
            let PlacedChild { element, layout } = &mut *child;
            window
                .with_retained_layout_async(
                    layout.as_mut().expect("positioned child layout"),
                    async |window| {
                        element
                            .prepaint_at_async(point(px(0.0), px(0.0)), window, cx, stack)
                            .await;
                    },
                )
                .await;
        }
    }

    fn paint(
        &mut self,
        bounds: Bounds<Pixels>,
        layout: &mut (),
        prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        crate::hooks::paint_on_stack(self, bounds, layout, prepaint, window, cx);
    }

    #[expect(
        clippy::await_holding_refcell_ref,
        reason = "exclusive slot access spans same-thread PaintStack child polls; no executor yield"
    )]
    fn paint_async<'a>(
        &'a mut self,
        _bounds: Bounds<Pixels>,
        _layout: &'a mut (),
        _prepaint: &'a mut (),
        window: &'a mut Window,
        cx: &'a mut App,
        stack: &'a mut gpui::PaintStack,
    ) -> gpui::PaintFuture<'a> {
        Box::pin(async move {
            if self.placeholder.is_some() {
                let mut child = self.child.borrow_mut();
                let PlacedChild { element, layout } = &mut *child;
                window
                    .with_retained_layout_async(
                        layout.as_mut().expect("positioned child layout"),
                        async |window| {
                            crate::hooks::paint_reopen_async(window, async |window| {
                                element.paint_async(window, cx, stack).await;
                            })
                            .await;
                        },
                    )
                    .await;
            }
        })
    }
}
