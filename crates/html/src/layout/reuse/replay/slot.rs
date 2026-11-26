//! Место повтора блока в кадре и его проверка перед prepaint.

use super::super::frame::{raster_phase_matches, scaled};
use super::{Ranges, ReplayRecord, plain_environment};
use gpui::{ContentMask, LayoutId, Pixels, Point, Window};
use std::cell::Cell;
use std::rc::Rc;

/// Место повтора в кадре: узел раскладки, если дерево кадра его отдало.
pub(in crate::layout::reuse) struct ReplaySlot {
    pub(super) record: Rc<ReplayRecord>,
    pub(super) layout: Cell<Option<LayoutId>>,
    /// How far the block lies from where it painted (`valid`): a content
    /// record whose output moves (`Ranges::movable`) replays moved.
    pub(super) offset: Cell<Point<Pixels>>,
    /// The copy meets another mask than its record's and is clipped by it
    /// (`Replay::prepaint`).
    pub(super) cut: Cell<bool>,
}

impl ReplaySlot {
    pub(in crate::layout::reuse) fn new(record: Rc<ReplayRecord>) -> Self {
        Self {
            record,
            layout: Cell::new(None),
            offset: Cell::new(Point::default()),
            cut: Cell::new(false),
        }
    }

    /// DOM node of the replayed block.
    pub(in crate::layout::reuse) fn node(&self) -> u64 {
        self.record.source.id()
    }

    /// This frame's replay is exact (`None`): its nodes are claimed, no measure
    /// was needed, the block lies where it was recorded at the same size in the
    /// same environment, and its ranges were recorded in the frame just shown;
    /// otherwise the refusal code for the `why` journal.
    pub(in crate::layout::reuse) fn refusal(&self, window: &mut Window) -> Option<&'static str> {
        let Some(id) = self.layout.get() else {
            return Some("unclaimed");
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
        // A box's content (`content`, no flow key) or an inert root-flow
        // block may lie elsewhere: its movable output replays moved.
        let content = self.record.key.is_none();
        let moves = content || self.record.inert;
        let placed = self.record.ranges(|r| {
            // Its leaves keep their functions: one measured again may lie or
            // draw otherwise (`Window::retained_remeasured`).
            let remeasured = content && window.retained_remeasured(&r.layout);
            let moved = r.origin != origin;
            let refusal = match () {
                _ if remeasured => Some("remeasured"),
                _ if r.serial + 1 != serial => Some("serial"),
                _ if !r.lines.complete() => Some("lines"),
                _ if r.scale != scale => Some("scale"),
                _ if !clip_holds(r, window, id, origin, &mask) => Some("mask"),
                _ if r.size != size => Some("size"),
                _ if moved
                    && !(moves
                        && r.movable
                        && raster_phase_matches(
                            scaled(r.origin, scale),
                            scaled(origin, scale),
                        )) =>
                {
                    Some("moved")
                }
                _ => None,
            };
            (refusal, origin - r.origin)
        });
        match placed {
            Some((None, offset)) if fresh => {
                self.offset.set(offset);
                None
            }
            Some((None, _)) => Some("stale"),
            Some((refusal, _)) => refusal,
            None => Some("ranges"),
        }
    }
}

/// The record's mask still holds at `origin`: it is the frame's `mask`, or a
/// container's clip (`Ranges::container`) whose layout parent lies the same
/// size where the record moved with it, inside the frame's mask. The clip of
/// an ancestor above the parent is not known here: `Replay` clips by it.
fn clip_holds(
    r: &Ranges,
    window: &mut Window,
    id: LayoutId,
    origin: Point<Pixels>,
    mask: &ContentMask<Pixels>,
) -> bool {
    if r.mask == *mask {
        return true;
    }
    let (Some(container), Some(parent)) = (r.container, window.parent_layout_id(id)) else {
        return false;
    };
    let offset = origin - r.origin;
    let moved = gpui::Bounds {
        origin: r.mask.bounds.origin + offset,
        size: r.mask.bounds.size,
    };
    let shift = window.layout_origin_unrounded(parent) - container.origin;
    window.layout_size_unrounded(parent) == container.size
        && near(shift.x, offset.x)
        && near(shift.y, offset.y)
        && within(moved, mask.bounds)
}

/// Unrounded layout positions of one placement differ by float noise only;
/// their device edges round alike (`raster_phase_matches`).
fn near(a: Pixels, b: Pixels) -> bool {
    (a - b).abs() < gpui::px(1.0 / 64.0)
}

/// `inner` lies inside `outer` but for float noise (`near`).
pub(super) fn within(inner: gpui::Bounds<Pixels>, outer: gpui::Bounds<Pixels>) -> bool {
    let (a, b) = (inner, outer);
    let ge = |x: Pixels, y: Pixels| x > y || near(x, y);
    ge(a.origin.x, b.origin.x)
        && ge(a.origin.y, b.origin.y)
        && ge(b.origin.x + b.size.width, a.origin.x + a.size.width)
        && ge(b.origin.y + b.size.height, a.origin.y + a.size.height)
}

/// `a` and `b` are the same bounds but for float noise (`near`).
pub(super) fn same(a: gpui::Bounds<Pixels>, b: gpui::Bounds<Pixels>) -> bool {
    within(a, b) && within(b, a)
}
