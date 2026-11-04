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

use super::{Counters, FlowKey};
use crate::dom::Element;
use crate::hooks::{Hooks, Wrap};
use gpui::{
    AnyElement, App, Bounds, ContentMask, InkSink, LayoutId, Pixels, Point, RetainedPaint,
    RetainedPrepaint, RetainedSubtree, Size, Window, point, px,
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
    pub(super) start: Counters,
    pub(super) end: Counters,
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
            && (!self.widths || self.key.same_widths(key))
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
            sink: Rc::default(),
        }
    }

    /// Готовая запись блока `element`, если его prepaint и краска переносимы.
    pub(super) fn finish(&self, element: Box<Element>) -> Option<ReplayRecord> {
        if self.sink.tainted() {
            return None;
        }
        let (layout, _) = self.layout.take()?;
        let (prepaint, origin, size) = self.prepaint.take()?;
        let paint = self.paint.take()?;
        let (scale, serial) = self.env.get()?;
        let mask = self.mask.take()?;
        Some(ReplayRecord {
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

/// Прозрачная обёртка блока: снимает узлы раскладки, диапазоны prepaint и
/// краски этого кадра.
pub(super) struct ReplayRecorder {
    child: AnyElement,
    pending: Rc<PendingReplay>,
}

impl ReplayRecorder {
    pub(super) fn new(child: AnyElement, pending: Rc<PendingReplay>) -> Self {
        Self { child, pending }
    }
}

impl Wrap for ReplayRecorder {
    fn paint_boundary(&self) -> bool {
        false
    }

    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> LayoutId {
        let start = window.layout_request_position();
        let id = self.child.request_layout(window, cx);
        let layout = window.capture_layout(start, id);
        self.pending
            .layout
            .replace(layout.map(|layout| (layout, id)));
        id
    }

    fn prepaint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let callbacks = window.next_frame_callback_count();
        let plain = plain_environment(window);
        let (_, range) = window.record_retained_prepaint(|window| {
            let _ = self.child.prepaint(window, cx);
        });
        let quiet =
            range.quiet(window).is_some() && callbacks == window.next_frame_callback_count();
        let Some(id) = self.pending.layout.borrow().as_ref().map(|(_, id)| *id) else {
            return;
        };
        if plain && quiet {
            let origin = window.layout_origin_unrounded(id);
            let size = window.layout_size_unrounded(id);
            self.pending.prepaint.replace(Some((range, origin, size)));
            self.pending.mask.replace(Some(window.content_mask()));
        }
    }

    fn paint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let sink = self.pending.sink.clone();
        let callbacks = window.next_frame_callback_count();
        let (_, paint) = window.with_ink_sink(sink, |window| {
            window.record_retained_paint(|window| self.child.paint(window, cx))
        });
        let prepaint = self
            .pending
            .prepaint
            .borrow()
            .as_ref()
            .map(|(p, _, _)| p.clone());
        let replayable = prepaint
            .is_some_and(|prepaint| window.retained_replayable(&prepaint, &paint))
            && callbacks == window.next_frame_callback_count();
        if replayable {
            self.pending.paint.replace(Some(paint));
            self.pending
                .env
                .set(Some((window.scale_factor(), window.frame_serial())));
        }
    }
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

/// Заместитель блока: занятые узлы раскладки, повтор prepaint и краски.
pub(super) struct Replay {
    slot: Rc<ReplaySlot>,
}

impl Replay {
    pub(super) fn new(slot: Rc<ReplaySlot>) -> Self {
        Self { slot }
    }
}

impl Hooks for Replay {
    fn paint_boundary(&self) -> bool {
        false
    }

    type Layout = ();
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        let layout = self.slot.record.ranges(|r| r.layout.clone());
        match layout.and_then(|layout| window.request_retained_layout(&layout)) {
            Some(id) => {
                self.slot.layout.set(Some(id));
                (id, ())
            }
            // Узлы ушли: проверка кадра отвергнет повтор.
            None => (window.request_layout(gpui::Style::default(), [], cx), ()),
        }
    }

    fn prepaint(&mut self, _: Bounds<Pixels>, _: &mut (), window: &mut Window, _: &mut App) {
        self.slot
            .record
            .ranges(|r| r.prepaint = window.replay_retained_prepaint(&r.prepaint));
    }

    fn paint(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        _: &mut App,
    ) {
        self.slot.record.ranges(|r| {
            r.paint = window.replay_retained_paint(&r.paint);
            r.serial = window.frame_serial();
        });
    }
}
