//! Запись блока: прозрачная обёртка снимает лист раскладки, начало коробки,
//! инертность prepaint и чернила краски, включая отложенную краску строк.

use super::lines::{LineSink, with_sink};
use super::width::{Width, reusable_range};
use super::{Counters, FlowKey, Record, Replay, placed, scaled};
use crate::dom::Element;
use crate::hooks::{LineCapture, Wrap};
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
    /// Зависимость раскладки блока от ширины потока (`width::independent`).
    width: Width,
    /// Ключ потока прошлой сборки тот же: запись годна хотя бы при нём.
    /// Иначе (ширина меняется) пишется лишь запись, годная и при другой.
    settled: bool,
    /// Ширина рамки блока в этом кадре.
    border: Cell<f32>,
    /// Строки абзацев блока в этом кадре.
    lines: Rc<LineSink>,
    /// Вывод prepaint и краски этого кадра и маска краски.
    prepaint: RefCell<Option<RetainedPrepaint>>,
    paint: RefCell<Option<Painted>>,
}

/// Вывод краски блока, его отложенные строки и маска краски.
type Painted = (RetainedPaint, Rc<LineCapture>, Bounds<Pixels>);

impl Pending {
    pub(super) fn new(
        key: Rc<FlowKey>,
        start: Counters,
        end: Counters,
        width: Width,
        settled: bool,
    ) -> Self {
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
            width,
            settled,
            border: Cell::new(0.0),
            lines: Rc::default(),
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
            (Some(prepaint), Some((paint, deferred, at)))
                if at == mask && paint.replayable() && deferred.complete() =>
            {
                Some(Replay {
                    prepaint,
                    paint,
                    deferred,
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
        let resizable = reusable_range(
            self.width,
            self.border.get(),
            self.key.constraints.avail_w,
            self.lines.slack(),
        );
        if !self.settled && resizable.is_none() {
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
            resizable,
            // Блок шире потока держит пол (`Width::Floor`), а не поток.
            stretch: self.width == Width::Stretch
                && self
                    .key
                    .constraints
                    .avail_w
                    .is_some_and(|flow| self.border.get() <= flow + 0.5),
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

    fn prepaint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        self.pending.border.set(bounds.size.width.as_f32());
        let callbacks = window.next_frame_callback_count();
        let root = window.retained_root_environment()
            && window.element_offset() == point(px(0.0), px(0.0));
        let lines = self.pending.lines.clone();
        let (_, range) = window.record_retained_prepaint(|window| {
            with_sink(lines, window, |window| self.child.prepaint(window, cx));
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
        // Строки блока рисуются в конце кадра стопки (`paint::coordinator`).
        let ((_, paint), deferred) = crate::hooks::paint_capturing(window, |window| {
            window.with_ink_sink(sink, |window| {
                window.record_retained_paint(|window| self.child.paint(window, cx))
            })
        });
        self.pending.paint.replace(Some((paint, deferred, mask)));
        self.pending.painted.set(true);
    }
}
