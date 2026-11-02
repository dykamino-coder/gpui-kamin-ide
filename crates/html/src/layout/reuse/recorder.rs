//! Запись блока: прозрачная обёртка снимает лист раскладки, начало коробки,
//! инертность prepaint и чернила краски, включая отложенную краску строк.

use super::{Counters, FlowKey, Record, scaled};
use crate::dom::Element;
use crate::hooks::Wrap;
use gpui::{
    AnyElement, App, Bounds, InkSink, LayoutId, LayoutSnapshot, Pixels, Point, ScaledPixels,
    Window, point, px, size,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Запись, которая завершится после краски кадра (`FlowReuse::finish`).
pub(super) struct Pending {
    element: Element,
    key: Rc<FlowKey>,
    start: Counters,
    end: Counters,
    layout: Cell<Option<LayoutId>>,
    snapshot: RefCell<Option<LayoutSnapshot>>,
    origin: Cell<Option<Point<ScaledPixels>>>,
    /// Простые hitbox поддерева: пропуск теряет их только вне экрана.
    hits: Cell<Option<Bounds<ScaledPixels>>>,
    quiet: Cell<bool>,
    painted: Cell<bool>,
    sink: Rc<InkSink>,
}

impl Pending {
    pub(super) fn new(element: Element, key: Rc<FlowKey>, start: Counters, end: Counters) -> Self {
        Self {
            element,
            key,
            start,
            end,
            layout: Cell::new(None),
            snapshot: RefCell::new(None),
            origin: Cell::new(None),
            hits: Cell::new(None),
            quiet: Cell::new(false),
            painted: Cell::new(false),
            sink: Rc::default(),
        }
    }

    /// Готовая запись, если блок прошёл prepaint и краску инертно.
    pub(super) fn finish(&self, screen: Bounds<ScaledPixels>) -> Option<Record> {
        if !self.quiet.get() || !self.painted.get() || self.sink.tainted() {
            return None;
        }
        let origin = self.origin.get()?;
        let ink = match (self.sink.bounds(), self.hits.get()) {
            (Some(a), Some(b)) => Some(a.union(&b)),
            (a, b) => a.or(b),
        };
        let ink = ink.map(|ink| Bounds {
            origin: point(
                ScaledPixels(ink.origin.x.0 - origin.x.0),
                ScaledPixels(ink.origin.y.0 - origin.y.0),
            ),
            size: ink.size,
        });
        Some(Record {
            element: self.element.clone(),
            key: self.key.clone(),
            start: self.start,
            end: self.end,
            snapshot: self.snapshot.borrow_mut().take()?,
            ink,
            origin,
            screen,
        })
    }
}

pub(super) struct Recorder {
    child: AnyElement,
    pending: Rc<Pending>,
}

impl Recorder {
    pub(super) fn new(child: AnyElement, pending: Rc<Pending>) -> Self {
        Self { child, pending }
    }
}

impl Wrap for Recorder {
    fn paint_boundary(&self) -> bool {
        false
    }

    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> LayoutId {
        let id = self.child.request_layout(window, cx);
        self.pending.layout.set(Some(id));
        id
    }

    fn prepaint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let callbacks = window.next_frame_callback_count();
        let root = window.retained_root_environment()
            && window.element_offset() == point(px(0.0), px(0.0));
        let (_, range) = window.record_retained_prepaint(|window| {
            let _ = self.child.prepaint(window, cx);
        });
        let hits = range
            .quiet(window)
            .filter(|_| root && callbacks == window.next_frame_callback_count());
        let (Some(id), Some(hits)) = (self.pending.layout.get(), hits) else {
            return;
        };
        let scale = window.scale_factor();
        self.pending.hits.set(hits.map(|b| Bounds {
            origin: scaled(b.origin, scale),
            size: size(
                ScaledPixels(b.size.width.as_f32() * scale),
                ScaledPixels(b.size.height.as_f32() * scale),
            ),
        }));
        let snapshot = window.layout_snapshot(id);
        let origin = scaled(window.layout_origin_unrounded(id), scale);
        self.pending.quiet.set(snapshot.is_some());
        *self.pending.snapshot.borrow_mut() = snapshot;
        self.pending.origin.set(Some(origin));
    }

    fn paint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let sink = self.pending.sink.clone();
        window.with_ink_sink(sink, |window| self.child.paint(window, cx));
        self.pending.painted.set(true);
    }
}
