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

pub(super) use element::{Replay, ReplayRecorder};

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
    pub(super) element: Box<Element>,
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

/// Запись, которая завершится после краски кадра (`FlowReuse::finish`).
pub(super) struct PendingReplay {
    key: Rc<FlowKey>,
    start: Counters,
    end: Counters,
    /// Чтения ширин снаружи за сборку и меры кадра записи.
    pub(super) widths: Rc<Cell<bool>>,
    layout: RefCell<Option<(RetainedSubtree, LayoutId)>>,
    prepaint: RefCell<Option<(RetainedPrepaint, Point<Pixels>, Size<Pixels>)>>,
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
        element: Box<Element>,
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

/// Место повтора в кадре: узел раскладки, если дерево кадра его отдало.
pub(super) struct ReplaySlot {
    record: Rc<ReplayRecord>,
    layout: Cell<Option<LayoutId>>,
}

impl ReplaySlot {
    pub(super) fn new(record: Rc<ReplayRecord>) -> Self {
        Self {
            record,
            layout: Cell::new(None),
        }
    }

    /// DOM node of the replayed block.
    pub(super) fn node(&self) -> u64 {
        self.record.element.node_id
    }

    /// Повтор этого кадра точен: узлы заняты, ни одна мера не понадобилась,
    /// блок лёг на записанное место того же размера в том же окружении, а
    /// диапазоны записаны в только что показанном кадре.
    pub(super) fn valid(&self, window: &mut Window) -> bool {
        let Some(id) = self.layout.get() else {
            return false;
        };
        let fresh = !window.layout_stale() && plain_environment(window);
        let (origin, size) = (
            window.layout_origin_unrounded(id),
            window.layout_size_unrounded(id),
        );
        let (serial, scale, mask) = (
            window.frame_serial(),
            window.scale_factor(),
            window.content_mask(),
        );
        fresh
            && self.record.jobs.iter().all(|job| job.ready())
            && self
                .record
                .ranges(|r| {
                    r.serial + 1 == serial
                        && r.scale == scale
                        && r.mask == mask
                        && r.origin == origin
                        && r.size == size
                })
                .unwrap_or(false)
    }
}
