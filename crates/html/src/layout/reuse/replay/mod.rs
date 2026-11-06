//! Повтор блока корневого потока НА ЭКРАНЕ без сборки его элементов.
//!
//! Блок, чьё поддерево равно записанному (`dom::same_tree`), чей поток имеет
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
mod slot;

pub(super) use element::{Replay, ReplayRecorder};
pub(super) use slot::ReplaySlot;

use super::{Counters, FlowKey};
use crate::dom::Element;
use crate::paint::coordinator::{JobSlot, JobTap};
use gpui::{
    Bounds, ContentMask, InkSink, LayoutId, Pixels, Point, RetainedPaint, RetainedPrepaint,
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
}

/// Запись повтора: вход сборки блока и диапазоны его последнего кадра.
pub(crate) struct ReplayRecord {
    pub(super) element: Rc<Element>,
    pub(super) key: Rc<FlowKey>,
    /// Сборка прочла ширину снаружи: запись годится только при тех же ширинах.
    pub(super) widths: bool,
    /// Блок заполняет ширину потока (`plain::fills_width`): при другой ширине
    /// он другого размера, и повтор не пробуется.
    fills: bool,
    pub(super) start: Counters,
    pub(super) end: Counters,
    /// Задания строк блока в фазе строк контекста (`paint::coordinator::tap_jobs`).
    jobs: Rc<[Rc<JobSlot>]>,
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

    /// Запись подходит потоку `key` при счётчиках `start` и поддереве `e`.
    pub(super) fn fits(&self, key: &FlowKey, start: Counters, e: &Element) -> bool {
        self.key.same_outside_widths(key)
            && (!(self.widths || self.fills) || self.key.same_widths(key))
            && self.start == start
            && crate::dom::same_tree(&self.element, e)
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
    layout: RefCell<Option<(RetainedSubtree, LayoutId)>>,
    prepaint: RefCell<Option<PlacedPrepaint>>,
    paint: RefCell<Option<RetainedPaint>>,
    env: Cell<Option<(f32, u64)>>,
    mask: RefCell<Option<ContentMask<Pixels>>>,
    jobs: RefCell<Option<Rc<JobTap>>>,
    sink: Rc<InkSink>,
}

impl PendingReplay {
    pub(super) fn new(
        key: Rc<FlowKey>,
        start: Counters,
        end: Counters,
        widths: Rc<Cell<bool>>,
    ) -> Self {
        Self {
            key,
            start,
            end,
            widths,
            layout: RefCell::new(None),
            prepaint: RefCell::new(None),
            paint: RefCell::new(None),
            env: Cell::new(None),
            mask: RefCell::new(None),
            jobs: RefCell::new(None),
            sink: Rc::default(),
        }
    }

    /// Готовая запись блока `element`, если его prepaint и краска переносимы
    /// и он был у экрана `screen` (чернилами, а без них коробкой): дальний
    /// блок пропускает запись пропуска (`recorder::far`), она дешевле повтора.
    pub(super) fn finish(
        &self,
        element: Rc<Element>,
        screen: Bounds<ScaledPixels>,
    ) -> Option<ReplayRecord> {
        if self.sink.tainted() {
            return None;
        }
        let (layout, _) = self.layout.take()?;
        let (prepaint, origin, size) = self.prepaint.take()?;
        let paint = self.paint.take()?;
        let (scale, serial) = self.env.get()?;
        let ink = self
            .sink
            .bounds()
            .unwrap_or(Bounds { origin, size }.scale(scale));
        if super::recorder::far(Some(ink), Point::default(), screen) {
            return None;
        }
        let mask = self.mask.take()?;
        let jobs = self.jobs.take()?.finish()?;
        Some(ReplayRecord {
            jobs,
            fills: super::plain::fills_width(&element),
            element,
            key: self.key.clone(),
            widths: self.widths.get(),
            start: self.start,
            end: self.end,
            ranges: Cell::new(Some(Ranges {
                layout,
                prepaint,
                paint,
                origin,
                size,
                mask,
                scale,
                serial,
            })),
        })
    }
}

/// Окружение, в котором краска переносима как есть: корень окна без сдвига.
fn plain_environment(window: &Window) -> bool {
    window.retained_root_environment() && window.element_offset() == point(px(0.0), px(0.0))
}

impl super::FlowReuse {
    /// Проверка повторов кадра (`ReplaySlot::valid`); неудачные блоки (все,
    /// если раскладке понадобилась мера) запрещаются, пока ключ потока тот же.
    pub(super) fn validate_replays(&self, window: &mut Window) -> bool {
        let slots = self.replay_slots.take();
        let failed: Vec<u64> = slots
            .iter()
            .filter(|slot| !slot.valid(window))
            .map(|slot| slot.node())
            .collect();
        if !failed.is_empty() {
            // A stale layout does not say which block needed a measurement.
            let all = window.layout_stale();
            let mut banned = self.store.banned.take();
            banned.extend(
                slots
                    .iter()
                    .map(|slot| slot.node())
                    .filter(|n| all || failed.contains(n)),
            );
            self.store.banned.replace(banned);
            return false;
        }
        if !slots.is_empty() {
            let mut stats = self.store.stats.get();
            stats.replayed += slots.len() as u64;
            self.store.stats.set(stats);
        }
        true
    }

    /// Записи повтора этого кадра: повторённые и завершённые у экрана `screen`.
    pub(super) fn finish_replays(&self, screen: Bounds<ScaledPixels>) {
        let mut replays = self.next_replays.take();
        let mut reads = self.store.reads_widths.take();
        for (pending, element) in self.pending_replays.take() {
            if pending.widths.get() {
                reads.insert(element.node_id);
            } else {
                reads.remove(&element.node_id);
            }
            if let Some(record) = pending.finish(element, screen) {
                replays.insert(record.element.node_id, Rc::new(record));
            }
        }
        self.store.reads_widths.replace(reads);
        self.store.replays.replace(replays);
    }
}
