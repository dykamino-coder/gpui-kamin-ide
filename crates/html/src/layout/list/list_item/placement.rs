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

    pub(super) fn marker(&self, child: AnyElement, vertical: bool) -> AnyElement {
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
        let anchor = self.content.get().and_then(|id| {
            let baseline = window.first_content_baseline(id, self.axis)?;
            Some(window.layout_origin_unrounded(id).along(self.axis) + baseline)
        });
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
