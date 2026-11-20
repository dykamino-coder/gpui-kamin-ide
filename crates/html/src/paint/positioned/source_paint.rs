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
///
/// `node` is the scroll node of the containing block's layer (`layer_node`):
/// the source half prepaints the box in that node's coordinates, not in
/// those of its tree position, so the box scrolls with its containing block
/// whatever the offset the frame is built at.
pub(crate) fn split(
    spot: SpotCell,
    child: AnyElement,
    node: Option<gpui::ScrollHandle>,
) -> (AnyElement, AnyElement) {
    let child = Rc::new(RefCell::new(PlacedChild {
        element: LatePlace { child, spot }.hooked(),
        layout: None,
        node,
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
    node: Option<gpui::ScrollHandle>,
}

impl Hooks for SourcePaint {
    type Layout = ();
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        let layout = match self.placeholder.as_mut() {
            Some(placeholder) => placeholder.request_layout(window, cx),
            None => {
                let mut child = self.child.borrow_mut();
                child.layout = Some(window.retained_layout());
                child.element.request_layout(window, cx)
            }
        };
        (layout, ())
    }

    fn prepaint(&mut self, _: Bounds<Pixels>, _: &mut (), window: &mut Window, cx: &mut App) {
        if self.placeholder.is_some() {
            let mut child = self.child.borrow_mut();
            let PlacedChild {
                element,
                layout,
                node,
            } = &mut *child;
            // The layer's space: the element offset of its scroll node's
            // children, or the window's outside any scroll node.
            let origin = node
                .as_ref()
                .and_then(|node| window.scroll_node_children_offset(node))
                .unwrap_or(point(px(0.0), px(0.0)));
            window.with_retained_layout(
                layout.as_mut().expect("positioned child layout"),
                |window| {
                    element.prepaint_at(origin, window, cx);
                },
            );
        }
    }

    fn paint(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.placeholder.is_some() {
            let mut child = self.child.borrow_mut();
            let PlacedChild {
                element, layout, ..
            } = &mut *child;
            window.with_retained_layout(
                layout.as_mut().expect("positioned child layout"),
                |window| {
                    crate::hooks::paint_reopen(window, |window| element.paint(window, cx));
                },
            );
        }
    }
}
