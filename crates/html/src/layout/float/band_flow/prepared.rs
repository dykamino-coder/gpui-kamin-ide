//! Измеренный ребёнок и его дерево GPUI: держатель переносит их без новой сборки DOM.

use crate::hooks::{Hooks, IntoHooked};
use gpui::{
    AnyElement, App, AvailableSpace, Bounds, ElementIdContext, LayoutId, Pixels, Point,
    RetainedLayout, Size, Window, point, px, size,
};
use std::cell::Cell;
use std::rc::Rc;

pub(super) type Engine = RetainedLayout;

#[derive(Clone, Default)]
pub(super) struct Prepared(Rc<Cell<Option<Child>>>);

struct Child {
    element: AnyElement,
    engine: Engine,
    identity: ElementIdContext,
    space: Size<AvailableSpace>,
    size: Size<Pixels>,
    offset: Option<Point<Pixels>>,
}

impl std::fmt::Debug for Prepared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let child = self.0.take();
        let present = child.is_some();
        self.0.set(child);
        f.debug_tuple("Prepared").field(&present).finish()
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
        Self(Rc::new(Cell::new(Some(Child {
            element,
            engine,
            identity,
            space,
            size,
            offset: None,
        }))))
    }

    pub(super) fn take(&self, offset: Option<Point<Pixels>>) -> Option<AnyElement> {
        // План и его копии разделяют только право однократной передачи:
        // после take ребёнком целиком владеет держатель текущего дерева.
        self.0.take().map(|mut child| {
            child.offset = offset;
            child.hooked()
        })
    }
}

impl Hooks for Child {
    type Layout = LayoutId;
    type Prepaint = ();

    fn paint_boundary(&self) -> bool {
        // This holder only transports layout ownership. The child's frame
        // must enter the enclosing CSS pass before applying its own atomic
        // float policy (CSS 2.1 Appendix E.2 step 5).
        false
    }

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, LayoutId) {
        // Сам держатель несёт offset; второй layout-узел не нужен.
        let (position, inset) = match self.offset {
            Some(offset) => (
                gpui::Position::Absolute,
                gpui::Edges {
                    left: offset.x.into(),
                    top: offset.y.into(),
                    ..gpui::Edges::auto()
                },
            ),
            None => (gpui::Position::Relative, gpui::Edges::auto()),
        };
        let style = gpui::Style {
            size: size(self.size.width.into(), self.size.height.into()),
            flex_shrink: 0.0,
            position,
            inset,
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
        window.with_retained_layout(&mut self.engine, |window| {
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
        window.with_retained_layout(&mut self.engine, |window| {
            window.with_element_id_context(&self.identity, |window| self.element.paint(window, cx))
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::IntoElement;

    #[gpui::test]
    fn clones_transfer_the_prepared_child_only_once(cx: &mut gpui::TestAppContext) {
        cx.add_empty_window().draw(
            point(px(0.), px(0.)),
            size(px(100.), px(100.)),
            |window, _| {
                let prepared = Prepared::new(
                    gpui::div().into_any_element(),
                    RetainedLayout::default(),
                    window.element_id_context(),
                    size(AvailableSpace::MaxContent, AvailableSpace::MaxContent),
                    size(px(20.), px(10.)),
                );
                let copy = prepared.clone();
                assert_eq!(format!("{prepared:?}"), "Prepared(true)");
                assert!(copy.take(None).is_some());
                assert_eq!(format!("{prepared:?}"), "Prepared(false)");
                assert!(prepared.take(None).is_none());
                assert!(copy.take(None).is_none());
                gpui::div()
            },
        );
    }
}
