//! Носители OOF и очередь слоя, живущая до подготовки кадра.

use super::{DeferredPaint, SpotCell};
use crate::hooks::{Hooks, IntoHooked};
use gpui::{AnyElement, App, AvailableSpace, Bounds, LayoutId, Pixels, Window, point, px, size};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::{Rc, Weak};

pub(crate) type BuildOof = Box<dyn FnOnce(Bounds<Pixels>, &mut Window, &mut App) -> AnyElement>;

/// Готовая геометрия остаётся в исходной раскладке; отложенная строится один раз.
pub(crate) enum OofKid {
    Ready(AnyElement),
    Deferred(BuildOof, Option<DeferredPaint>),
}

type Item = (SpotCell, OofKid);
type Place = fn(SpotCell, AnyElement) -> AnyElement;

#[derive(Clone, Default)]
pub(super) struct OofQueue(Rc<RefCell<VecDeque<Item>>>);

pub(super) struct Destination(Weak<RefCell<VecDeque<Item>>>);

impl Destination {
    pub(super) fn upgrade(&self) -> Option<OofQueue> {
        self.0.upgrade().map(OofQueue)
    }
}

impl OofQueue {
    pub(super) fn retain(&self) -> Destination {
        Destination(Rc::downgrade(&self.0))
    }

    pub(super) fn push(&self, spot: SpotCell, element: AnyElement) {
        self.push_kid(spot, OofKid::Ready(element));
    }

    pub(super) fn push_kid(&self, spot: SpotCell, kid: OofKid) {
        self.0.borrow_mut().push_back((spot, kid));
    }

    pub(super) fn is_empty(&self) -> bool {
        self.0.borrow().is_empty()
    }

    pub(super) fn place(self, place: Place) -> Vec<AnyElement> {
        let mut ready = Vec::new();
        let mut deferred = VecDeque::new();
        for (spot, kid) in self.0.take() {
            match kid {
                OofKid::Ready(element) => ready.push(place(spot, element)),
                OofKid::Deferred(build, paint) => {
                    deferred.push_back((spot, OofKid::Deferred(build, paint)));
                }
            }
        }
        *self.0.borrow_mut() = deferred;
        // Пустое назначение тоже нужно: выпавший потомок может прийти из
        // вложенного хоста позже. Слабые ссылки не образуют цикл с build.
        if !self.is_empty() || Rc::weak_count(&self.0) != 0 {
            ready.push(OofHost { queue: self, place }.hooked());
        }
        ready
    }
}

struct OofHost {
    queue: OofQueue,
    place: Place,
}

impl Hooks for OofHost {
    type Layout = ();
    type Prepaint = Vec<AnyElement>;

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        let mut style = gpui::Style {
            position: gpui::Position::Absolute,
            ..Default::default()
        };
        style.inset.top = px(0.).into();
        style.inset.left = px(0.).into();
        style.size.width = gpui::relative(1.).into();
        style.size.height = gpui::relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<AnyElement> {
        let mut built = Vec::new();
        loop {
            let item = self.queue.0.borrow_mut().pop_front();
            let Some((spot, kid)) = item else { break };
            // Выпавшие потомки текущего корня обходятся раньше следующего
            // OOF-соседа: его якорная зависимость может ссылаться на них.
            let pending = self.queue.0.take();
            let (child, paint) = match kid {
                OofKid::Ready(element) => (element, None),
                OofKid::Deferred(build, paint) => (build(bounds, window, cx), paint),
            };
            // Изолированный корень использует тот же layout engine, но уже
            // опубликованные рамки якорей текущего кадра.
            let child = (self.place)(spot, child);
            use gpui::{IntoElement, ParentElement, Styled};
            let mut root = gpui::div()
                .relative()
                .w(bounds.size.width)
                .h(bounds.size.height)
                .child(child)
                .into_any_element();
            root.layout_as_root_at(
                bounds.origin,
                size(
                    AvailableSpace::Definite(bounds.size.width),
                    AvailableSpace::Definite(bounds.size.height),
                ),
                window,
                cx,
            );
            root.prepaint_at(point(px(0.), px(0.)), window, cx);
            match paint {
                Some(paint) => paint.publish(root),
                None => built.push(root),
            }
            self.queue.0.borrow_mut().extend(pending);
        }
        built
    }

    fn paint(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut (),
        built: &mut Vec<AnyElement>,
        window: &mut Window,
        cx: &mut App,
    ) {
        gpui::paint_reopen(|| {
            for child in built {
                child.paint(window, cx);
            }
        });
    }
}
