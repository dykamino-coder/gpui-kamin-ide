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
use std::cell::Cell;
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
    /// The output draws the same pixels moved with its block
    /// (`Window::retained_movable`), and nothing of it was clipped.
    movable: bool,
}

/// Запись повтора: вход сборки блока и диапазоны его последнего кадра.
pub(crate) struct ReplayRecord {
    /// С чем сравнивается следующая сборка блока (`source`).
    pub(super) source: Source,
    /// Ключ корневого потока; у записи содержимого (`content`) его нет:
    /// она сверяет свой ключ сама.
    pub(super) key: Option<Rc<FlowKey>>,
    /// Сборка прочла ширину снаружи: запись годится только при тех же ширинах.
    pub(super) widths: bool,
    /// Блок заполняет ширину потока (`plain::fills_width`): при другой ширине
    /// он другого размера, и повтор не пробуется.
    fills: bool,
    pub(super) start: Counters,
    pub(super) end: Counters,
    /// The subtree takes no anchor and no step-8 key (`content::inert_tree`):
    /// it replays after any build that advanced the counters, advancing them
    /// by as many (`Counters::advanced`), and may lie elsewhere.
    pub(super) inert: bool,
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
        self.key.as_ref().is_some_and(|own| {
            own.same_outside_widths(key) && (!(self.widths || self.fills) || own.same_widths(key))
        }) && (self.start == start || self.inert)
            && (self.source.same(e) || self.source.same_unread_color(e))
    }
}

/// Prepaint блока с его неокруглённым началом и размером.
type PlacedPrepaint = (RetainedPrepaint, Point<Pixels>, Size<Pixels>);

/// Запись, которая завершится после краски кадра (`FlowReuse::finish`).
pub(super) struct PendingReplay {
    key: Option<Rc<FlowKey>>,
    start: Counters,
    end: Counters,
    inert: bool,
    /// Чтения ширин снаружи за сборку и меры кадра записи.
    pub(super) widths: Rc<Cell<bool>>,
    /// Блок заполняет ширину потока (`plain::fills_width`).
    fills: bool,
    paint_last: Option<u64>,
    /// What the frame captured, boxed apart: the element tree keeps the
    /// pending record until the next frame, but its capture goes to the
    /// record at `finish`.
    captured: Cell<Option<Box<Captured>>>,
    env: Cell<Option<(f32, u64)>>,
    /// The output moves with the block (`Ranges::movable`) but for its ink.
    movable: Cell<bool>,
    /// The block's ink sink in its build's list (`FlowReuse::ink`).
    sink: (Rc<InkSinkList>, u32),
}

/// The output of a recorded block in its frame, as it is captured.
#[derive(Default)]
pub(super) struct Captured {
    pub(super) layout: Option<(RetainedSubtree, LayoutId)>,
    pub(super) prepaint: Option<PlacedPrepaint>,
    pub(super) paint: Option<RetainedPaint>,
    pub(super) mask: Option<ContentMask<Pixels>>,
    pub(super) lines: Option<Rc<LineCapture>>,
}

impl PendingReplay {
    /// `f` over the capture, allocated at its first part.
    pub(super) fn capture<R>(&self, f: impl FnOnce(&mut Captured) -> R) -> R {
        let mut captured = self.captured.take().unwrap_or_default();
        let out = f(&mut captured);
        self.captured.set(Some(captured));
        out
    }

    /// `f` over the capture so far, if anything was captured.
    pub(super) fn captured<R>(&self, f: impl FnOnce(&Captured) -> Option<R>) -> Option<R> {
        let captured = self.captured.take()?;
        let out = f(&captured);
        self.captured.set(Some(captured));
        out
    }

    pub(super) fn new(
        key: Option<Rc<FlowKey>>,
        (start, end): (Counters, Counters),
        inert: bool,
        widths: Rc<Cell<bool>>,
        fills: bool,
        ink: &Rc<InkSinkList>,
        paint_last: Option<u64>,
    ) -> Self {
        Self {
            key,
            start,
            end,
            inert: inert && paint_last.is_none(),
            widths,
            fills,
            paint_last,
            captured: Cell::new(None),
            env: Cell::new(None),
            movable: Cell::new(false),
            sink: (ink.clone(), ink.open()),
        }
    }

    /// Готовая запись блока `source`, если его prepaint и краска переносимы
    /// и он был у экрана `screen` (чернилами, а без них коробкой): дальний
    /// блок пропускает запись пропуска (`recorder::far`), она дешевле повтора.
    pub(super) fn finish(
        &self,
        source: Source,
        screen: Option<Bounds<ScaledPixels>>,
    ) -> Option<ReplayRecord> {
        let (list, at) = &self.sink;
        let (bounds, tainted) = list.with(*at, |sink| (sink.bounds(), sink.tainted()))?;
        if tainted {
            return None;
        }
        let captured = *self.captured.take()?;
        let (layout, _) = captured.layout?;
        let (prepaint, origin, size) = captured.prepaint?;
        let paint = captured.paint?;
        let (scale, serial) = self.env.get()?;
        let ink = bounds.unwrap_or(Bounds { origin, size }.scale(scale));
        // Дальний блок корневого потока пропускает запись повтора: его
        // заместитель дешевле (`screen`); содержимое пишется везде.
        if screen.is_some_and(|screen| super::recorder::far(Some(ink), Point::default(), screen)) {
            return None;
        }
        let mask = captured.mask?;
        // A command clipped where it was recorded cannot be restored by a
        // moved copy: only a record whose ink lies inside its mask moves.
        let clip = mask.bounds.scale(scale);
        let movable = self.movable.get() && bounds.is_none_or(|ink| clip.intersect(&ink) == ink);
        let lines = captured.lines.filter(|lines| lines.complete())?;
        Some(ReplayRecord {
            fills: self.fills,
            source,
            key: self.key.clone(),
            widths: self.widths.get(),
            start: self.start,
            end: self.end,
            inert: self.inert,
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
                movable,
            })),
        })
    }
}

/// Окружение, в котором краска переносима как есть: корень окна без сдвига.
fn plain_environment(window: &Window) -> bool {
    window.retained_root_environment() && window.element_offset() == point(px(0.0), px(0.0))
}
