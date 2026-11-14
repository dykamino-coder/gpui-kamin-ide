//! Повтор блока корневого потока НА ЭКРАНЕ без сборки его элементов.
//!
//! Блок, чьё поддерево равно записанному (`Source::same`), чей поток имеет
//! тот же ключ вне ширин (`FlowKey::same_outside_widths`) и чья сборка не
//! прочла ни одной ширины снаружи (`BoxConstraints::track_widths`), строит те
//! же элементы, что в кадре записи: его узлы раскладки в дереве кадра GPUI
//! занимаются целиком (`Window::request_retained_layout`), а prepaint и
//! краска копируются из прошлого кадра. Ширина окна входит только в
//! раскладку: Taffy пересчитывает корень блока под новое доступное место и
//! берёт результаты детей из их кэшей. Если раскладке понадобилась новая мера
//! листа (`Window::layout_stale`) или блок встал не туда, где записан, кадр
//! собирается заново без повторов (`FlowReuse::validate`). Так LayoutNG
//! держит дерево LayoutObject и кэш результатов, а Blink `PaintController`
//! повторяет кэшированные подпоследовательности чистых поддеревьев.

mod element;
mod frame;
mod slot;

pub(super) use element::{Replay, ReplayRecorder};
pub(super) use slot::ReplaySlot;

use super::{Counters, FlowKey, Source};
use crate::dom::Element;
use crate::hooks::LineCapture;
use gpui::{
    Bounds, ContentMask, InkSinkList, LayoutId, Pixels, Point, RetainedPaint, RetainedPrepaint,
    RetainedSubtree, ScaledPixels, Size, Window, point, px,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Где и как блок лёг и нарисовался в кадре `serial`.
#[derive(Clone)]
pub(super) struct Ranges {
    layout: RetainedSubtree,
    prepaint: RetainedPrepaint,
    paint: RetainedPaint,
    origin: Point<Pixels>,
    size: Size<Pixels>,
    mask: ContentMask<Pixels>,
    scale: f32,
    serial: u64,
    /// Строки блока в фазе строк контекста (`paint::coordinator::capture`).
    lines: Rc<LineCapture>,
}

/// Запись повтора: вход сборки блока и диапазоны его последнего кадра.
pub(crate) struct ReplayRecord {
    /// С чем сравнивается следующая сборка блока (`source`).
    pub(super) source: Source,
    pub(super) key: Rc<FlowKey>,
    /// Сборка прочла ширину снаружи: запись годится только при тех же ширинах.
    pub(super) widths: bool,
    /// Блок заполняет ширину потока (`plain::fills_width`): при другой ширине
    /// он другого размера, и повтор не пробуется.
    fills: bool,
    pub(super) start: Counters,
    pub(super) end: Counters,
    /// The paint key of the block's step-8 wrapper, which goes on outside
    /// the records (`flow::held`).
    pub(super) paint_last: Option<u64>,
    /// Диапазоны последнего кадра; `None` только на время их правки.
    ranges: Cell<Option<Ranges>>,
}

impl ReplayRecord {
    /// `f` над диапазонами последнего кадра.
    fn ranges<R>(&self, f: impl FnOnce(&mut Ranges) -> R) -> Option<R> {
        let mut ranges = self.ranges.take()?;
        let out = f(&mut ranges);
        self.ranges.set(Some(ranges));
        Some(out)
    }

    /// Запись подходит потоку `key` при счётчиках `start` и поддереве `e`:
    /// тем же или отличным только `color`, которым ни одна его коробка не
    /// рисует (`Source::same_unread_color`, `super::unread_color`).
    pub(super) fn fits(&self, key: &FlowKey, start: Counters, e: &Element) -> bool {
        self.key.same_outside_widths(key)
            && (!(self.widths || self.fills) || self.key.same_widths(key))
            && self.start == start
            && (self.source.same(e) || self.source.same_unread_color(e))
    }
}

/// Prepaint блока с его неокруглённым началом и размером.
type PlacedPrepaint = (RetainedPrepaint, Point<Pixels>, Size<Pixels>);

/// Запись, которая завершится после краски кадра (`FlowReuse::finish`).
pub(super) struct PendingReplay {
    key: Rc<FlowKey>,
    start: Counters,
    end: Counters,
    /// Чтения ширин снаружи за сборку и меры кадра записи.
    pub(super) widths: Rc<Cell<bool>>,
    /// Блок заполняет ширину потока (`plain::fills_width`).
    fills: bool,
    paint_last: Option<u64>,
    /// The record is kept for a block far from the screen too (kept_far).
    keep_far: bool,
    layout: RefCell<Option<(RetainedSubtree, LayoutId)>>,
    prepaint: RefCell<Option<PlacedPrepaint>>,
    paint: RefCell<Option<RetainedPaint>>,
    env: Cell<Option<(f32, u64)>>,
    mask: RefCell<Option<ContentMask<Pixels>>>,
    lines: RefCell<Option<Rc<LineCapture>>>,
    /// The block's ink sink in its build's list (`FlowReuse::ink`).
    sink: (Rc<InkSinkList>, u32),
}

impl PendingReplay {
    pub(super) fn new(
        key: Rc<FlowKey>,
        start: Counters,
        end: Counters,
        widths: Rc<Cell<bool>>,
        fills: bool,
        ink: &Rc<InkSinkList>,
        paint_last: Option<u64>,
    ) -> Self {
        Self {
            key,
            start,
            end,
            widths,
            fills,
            paint_last,
            keep_far: false,
            layout: RefCell::new(None),
            prepaint: RefCell::new(None),
            paint: RefCell::new(None),
            env: Cell::new(None),
            mask: RefCell::new(None),
            lines: RefCell::new(None),
            sink: (ink.clone(), ink.open()),
        }
    }

    /// The record of a block that has no skip record to stand for it far
    /// from the screen (a nested replay area): `finish` keeps it there.
    pub(super) fn kept_far(mut self) -> Self {
        self.keep_far = true;
        self
    }

    /// Готовая запись блока `source`, если его prepaint и краска переносимы
    /// и он был у экрана `screen` (чернилами, а без них коробкой): дальний
    /// блок пропускает запись пропуска (`recorder::far`), она дешевле повтора;
    /// запись без неё (`kept_far`) остаётся и там.
    pub(super) fn finish(
        &self,
        source: Source,
        screen: Bounds<ScaledPixels>,
    ) -> Option<ReplayRecord> {
        let (list, at) = &self.sink;
        let (bounds, tainted) = list.with(*at, |sink| (sink.bounds(), sink.tainted()))?;
        if tainted {
            return None;
        }
        let (layout, _) = self.layout.take()?;
        let (prepaint, origin, size) = self.prepaint.take()?;
        let paint = self.paint.take()?;
        let (scale, serial) = self.env.get()?;
        let ink = bounds.unwrap_or(Bounds { origin, size }.scale(scale));
        if !self.keep_far && super::recorder::far(Some(ink), Point::default(), screen) {
            return None;
        }
        let mask = self.mask.take()?;
        let lines = self.lines.take().filter(|lines| lines.complete())?;
        Some(ReplayRecord {
            fills: self.fills,
            source,
            key: self.key.clone(),
            widths: self.widths.get(),
            start: self.start,
            end: self.end,
            paint_last: self.paint_last,
            ranges: Cell::new(Some(Ranges {
                layout,
                prepaint,
                paint,
                origin,
                size,
                mask,
                scale,
                serial,
                lines,
            })),
        })
    }
}

/// Окружение, в котором краска переносима как есть: корень окна без сдвига.
fn plain_environment(window: &Window) -> bool {
    window.retained_root_environment() && window.element_offset() == point(px(0.0), px(0.0))
}
