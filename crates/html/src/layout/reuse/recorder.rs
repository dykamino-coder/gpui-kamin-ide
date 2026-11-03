//! Запись блока: прозрачная обёртка снимает лист раскладки, начало коробки,
//! инертность prepaint и чернила краски, включая отложенную краску строк.

use super::{Counters, FlowKey, Record, Replay, placed, scaled};
use crate::dom::Element;
use crate::hooks::Wrap;
use gpui::{
    AnyElement, App, Bounds, InkSink, LayoutId, LayoutSnapshot, Pixels, Point, RetainedPaint,
    RetainedPrepaint, ScaledPixels, Window, point, px, size,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Запись, которая завершится после краски кадра (`FlowReuse::finish`). Её
/// держит и обёртка в дереве GPUI до следующего кадра, поэтому элемент блока
/// лежит не здесь, а у `FlowReuse` и переходит в запись без копии.
pub(super) struct Pending {
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
    /// Раскладка блока не зависит от ширины потока (`width::independent`).
    resizable: bool,
    /// Вывод prepaint и краски этого кадра и маска краски.
    prepaint: RefCell<Option<RetainedPrepaint>>,
    paint: RefCell<Option<(RetainedPaint, Bounds<Pixels>)>>,
}

impl Pending {
    pub(super) fn new(key: Rc<FlowKey>, start: Counters, end: Counters, resizable: bool) -> Self {
        Self {
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
            resizable,
            prepaint: RefCell::new(None),
            paint: RefCell::new(None),
        }
    }

    /// Готовая запись блока `element`, если он прошёл prepaint и краску
    /// инертно и либо был далеко от экрана `screen`, либо его вывод кадра
    /// `epoch` повторяем под маской корня `mask`.
    pub(super) fn finish(
        &self,
        element: Box<Element>,
        screen: Bounds<ScaledPixels>,
        mask: Bounds<Pixels>,
        epoch: u64,
    ) -> Option<Record> {
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
        let far = far(ink, origin, screen);
        let replay = match (self.prepaint.take(), self.paint.take()) {
            (Some(prepaint), Some((paint, at))) if at == mask && paint.replayable() => {
                Some(Replay {
                    prepaint,
                    paint,
                    origin,
                    mask,
                    epoch,
                })
            }
            _ => None,
        };
        if !far && replay.is_none() {
            return None;
        }
        let snapshot = self.snapshot.take()?;

        Some(Record {
            element,
            key: self.key.clone(),
            start: self.start,
            end: self.end,
            snapshot,
            ink,
            resizable: self.resizable,
            far,
            replay: RefCell::new(replay),
        })
    }
}

/// Блок с чернилами `ink` (относительно начала `origin`) был дальше экрана
/// `screen` по каждой оси. Сдвиг следующего кадра обычно меньше, а точный
/// ответ даёт `FlowReuse::validate`; ближний блок не пропустит ни один кадр,
/// и его запись только занимала бы память.
pub(super) fn far(
    ink: Option<Bounds<ScaledPixels>>,
    origin: Point<ScaledPixels>,
    screen: Bounds<ScaledPixels>,
) -> bool {
    let Some(ink) = ink else { return true };
    let s = screen.size;
    !placed(ink, origin, s.width.0.max(s.height.0)).intersects(&screen)
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
        self.pending.snapshot.replace(snapshot);
        self.pending.origin.set(Some(origin));
        self.pending.prepaint.replace(Some(range));
    }

    fn paint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let sink = self.pending.sink.clone();
        let mask = window.content_mask().bounds;
        let (_, paint) = window.with_ink_sink(sink, |window| {
            window.record_retained_paint(|window| self.child.paint(window, cx))
        });
        self.pending.paint.replace(Some((paint, mask)));
        self.pending.painted.set(true);
    }
}
