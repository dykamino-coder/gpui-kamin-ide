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
mod pending;
mod slot;

pub(super) use element::{Replay, ReplayRecorder};
pub(super) use frame::note_flow_refused;
pub(super) use pending::PendingReplay;
pub(super) use slot::ReplaySlot;

use super::{Counters, FlowKey, Source};
use crate::dom::Element;
use crate::hooks::LineCapture;
use gpui::{
    ContentMask, Pixels, Point, RetainedPaint, RetainedPrepaint, RetainedSubtree, Size, Window,
    point, px,
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
    /// A box's content drawn under a clip other than the frame's mask (a
    /// container's `overflow`): it moves with its block's clip and replays
    /// clipped by the mask it meets (`Replay`), while it is `movable`.
    clipped: bool,
    /// The hitboxes of its hover boxes, each with the hover it painted
    /// (`gpui::Window::retained_hovers`): the pointer is an input of the
    /// record only through them (`ReplayRecord::hovers_hold`).
    hovers: Box<[(gpui::HitboxId, bool)]>,
}

/// Запись повтора: вход сборки блока и диапазоны его последнего кадра.
pub(crate) struct ReplayRecord {
    /// С чем сравнивается следующая сборка блока (`source`).
    pub(super) source: Source,
    /// The root flow key; a content record (`content`) has none:
    /// it checks its own key.
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
    /// A clipped record whose copy an ancestor's clip cut no longer replays:
    /// its content is built (decided before layout, so no frame is refused).
    pub(super) fn replays_clipped(&self) -> bool {
        self.ranges(|r| !r.clipped || r.movable).unwrap_or(false)
    }

    /// Every hover box of the record shows the hover it painted
    /// (`gpui::HoverView::holds`), decided before layout: a record over a
    /// box the pointer entered or left is built, the others replay. So
    /// Blink invalidates the style of only the elements whose hover state
    /// changed and whose style is affected by it, and repaints them.
    pub(super) fn hovers_hold(&self, view: &gpui::HoverView) -> bool {
        self.ranges(|r| view.holds(&r.hovers)).unwrap_or(false)
    }

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

/// Окружение, в котором краска переносима как есть: корень окна без сдвига.
fn plain_environment(window: &Window) -> bool {
    window.retained_root_environment() && window.element_offset() == point(px(0.0), px(0.0))
}
