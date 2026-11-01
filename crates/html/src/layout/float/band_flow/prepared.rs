//! Измеренный ребёнок и его дерево GPUI: держатель переносит их без новой сборки DOM.

use crate::hooks::{Hooks, IntoHooked};
use gpui::{
    AnyElement, App, AvailableSpace, Bounds, ElementIdContext, LayoutId, Pixels, RetainedLayout,
    Size, Window, point, px, size,
};
use std::cell::RefCell;
use std::rc::Rc;

pub(super) type Engine = Rc<RefCell<RetainedLayout>>;

#[derive(Clone, Default)]
pub(super) struct Prepared(Rc<RefCell<Option<Child>>>);

struct Child {
    element: AnyElement,
    engine: Engine,
    identity: ElementIdContext,
    space: Size<AvailableSpace>,
    size: Size<Pixels>,
}

impl std::fmt::Debug for Prepared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Prepared")
            .field(&self.0.borrow().is_some())
            .finish()
    }
}

impl Prepared {
    pub(super) fn new(
        element: AnyElement,
        engine: Engine,
        identity: ElementIdContext,
        space: Size<AvailableSpace>,
        size: Size<Pixels>,
    ) -> Self {
        Self(Rc::new(RefCell::new(Some(Child {
            element,
            engine,
            identity,
            space,
            size,
        }))))
    }

    pub(super) fn take(&self) -> Option<AnyElement> {
        self.0.borrow_mut().take().map(|child| child.hooked())
    }
}

impl Hooks for Child {
    type Layout = LayoutId;
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, LayoutId) {
        let style = gpui::Style {
            size: size(self.size.width.into(), self.size.height.into()),
            flex_shrink: 0.0,
            ..Default::default()
        };
        let id = window.request_layout(style, [], cx);
        (id, id)
    }

    fn prepaint(
        &mut self,
        _bounds: Bounds<Pixels>,
        id: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let origin = window.layout_origin_unrounded(*id);
        window.with_retained_layout(&mut self.engine.borrow_mut(), |window| {
            window.with_element_id_context(&self.identity, |window| {
                self.element
                    .layout_as_root_at(origin, self.space, window, cx);
                self.element
                    .prepaint_at(point(px(0.0), px(0.0)), window, cx);
            });
        });
    }

    fn paint(
        &mut self,
        _bounds: Bounds<Pixels>,
        _id: &mut LayoutId,
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        window.with_retained_layout(&mut self.engine.borrow_mut(), |window| {
            window.with_element_id_context(&self.identity, |window| self.element.paint(window, cx))
        });
    }
}
