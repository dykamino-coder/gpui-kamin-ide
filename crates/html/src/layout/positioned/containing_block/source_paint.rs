//! Keep positioned painting in tree order while laying out at the containing block.

use super::{LatePlace, SpotCell};
use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Styled, Window, div, point, px,
};
use std::{cell::RefCell, rc::Rc};

/// CSS 2 Appendix E orders painting independently of the containing block.
/// Only the layout half belongs to the CB; the source half owns prepaint/paint.
pub(crate) fn split(spot: SpotCell, child: AnyElement) -> (AnyElement, AnyElement) {
    let child = Rc::new(RefCell::new(
        LatePlace {
            child: Some(child),
            spot,
        }
        .into_any_element(),
    ));
    let layout = SourcePaint {
        child: child.clone(),
        placeholder: None,
    };
    let paint = SourcePaint {
        child,
        placeholder: Some(div().absolute().w_0().h_0().into_any_element()),
    };
    (layout.into_any_element(), paint.into_any_element())
}

struct SourcePaint {
    child: Rc<RefCell<AnyElement>>,
    placeholder: Option<AnyElement>,
}

impl Element for SourcePaint {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let layout = match self.placeholder.as_mut() {
            Some(placeholder) => placeholder.request_layout(window, cx),
            None => self.child.borrow_mut().request_layout(window, cx),
        };
        (layout, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.placeholder.is_some() {
            self.child
                .borrow_mut()
                .prepaint_at(point(px(0.0), px(0.0)), window, cx);
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.placeholder.is_some() {
            gpui::paint_reopen(|| self.child.borrow_mut().paint(window, cx));
        }
    }
}

impl IntoElement for SourcePaint {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
