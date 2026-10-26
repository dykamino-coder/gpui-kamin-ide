//! Владелец внешнего маркера — фрагмент с первой содержательной baseline пункта.

use crate::hooks::{IntoHooked, Wrap};
use gpui::{Along, AnyElement, App, Axis, Bounds, LayoutId, Pixels, Window, point, px};
use std::cell::Cell;
use std::rc::Rc;

#[derive(Clone, Default)]
pub(super) struct Anchor(Rc<Cell<Option<LayoutId>>>);

impl Anchor {
    pub(super) fn content(&self, child: AnyElement) -> AnyElement {
        Content {
            child,
            id: self.0.clone(),
        }
        .hooked()
    }

    pub(super) fn marker(&self, child: AnyElement, vertical: bool, has_lines: bool) -> AnyElement {
        Marker {
            child,
            content: self.0.clone(),
            id: None,
            axis: if vertical {
                Axis::Horizontal
            } else {
                Axis::Vertical
            },
            owns: true,
            has_lines,
        }
        .hooked()
    }
}

struct Content {
    child: AnyElement,
    id: Rc<Cell<Option<LayoutId>>>,
}

impl Wrap for Content {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> LayoutId {
        let id = self.child.request_layout(window, cx);
        self.id.set(Some(id));
        id
    }
}

struct Marker {
    child: AnyElement,
    content: Rc<Cell<Option<LayoutId>>>,
    id: Option<LayoutId>,
    axis: Axis,
    owns: bool,
    has_lines: bool,
}

impl Marker {
    fn baseline(&self, window: &mut Window) -> Option<Pixels> {
        // Распорка пустого пункта задаёт высоту, но не создаёт строку
        // содержимого: такой маркер выровнен по верху (css-lists-3 §3.5).
        self.content
            .get()
            .filter(|_| self.has_lines)
            .and_then(|id| {
                let baseline = window.first_content_baseline(id, self.axis)?;
                Some(window.layout_origin_unrounded(id).along(self.axis) + baseline)
            })
    }
}

impl Wrap for Marker {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> LayoutId {
        let id = self.child.request_layout(window, cx);
        self.id = Some(id);
        id
    }

    fn prepaint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let mut delta = point(px(0.), px(0.));
        let anchor = self.baseline(window);
        if let Some(anchor) = anchor {
            let own = self
                .id
                .and_then(|id| window.first_content_baseline(id, self.axis));
            if let Some(own) = own {
                delta =
                    delta.apply_along(self.axis, |_| anchor - own - bounds.origin.along(self.axis));
            }
            let mask = window.content_mask().bounds;
            // Полуоткрытая граница даёт строке ровно одного владельца.
            self.owns = anchor >= mask.origin.along(self.axis)
                && anchor < mask.origin.along(self.axis) + mask.size.along(self.axis);
        } else {
            self.owns = true;
        }
        if self.owns {
            window.with_element_offset(delta, |window| self.child.prepaint(window, cx));
        }
    }

    fn paint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        if self.owns {
            self.child.paint(window, cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{AvailableSpace, IntoElement, ParentElement, Styled, div, size};

    #[gpui::test]
    fn empty_marker_regression_ignores_the_height_strut_baseline(cx: &mut gpui::TestAppContext) {
        cx.add_empty_window().draw(
            point(px(0.), px(0.)),
            size(px(100.), px(100.)),
            |window, app| {
                let mut strut = div().text_size(px(16.)).child("\u{a0}").into_any_element();
                let id = strut.request_layout(window, app);
                window.compute_layout(
                    id,
                    size(
                        AvailableSpace::Definite(px(100.)),
                        AvailableSpace::MaxContent,
                    ),
                    app,
                );
                assert!(window.first_content_baseline(id, Axis::Vertical).is_some());
                let mut marker = Marker {
                    child: div().into_any_element(),
                    content: Rc::new(Cell::new(Some(id))),
                    id: None,
                    axis: Axis::Vertical,
                    owns: true,
                    has_lines: false,
                };
                assert_eq!(marker.baseline(window), None);
                marker.has_lines = true;
                assert!(marker.baseline(window).is_some());
                div()
            },
        );
    }
}
