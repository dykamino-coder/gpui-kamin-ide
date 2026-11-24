//! Место повтора блока в кадре и его проверка перед prepaint.

use super::super::frame::{raster_phase_matches, scaled};
use super::{ReplayRecord, plain_environment};
use gpui::{LayoutId, Pixels, Point, Window};
use std::cell::Cell;
use std::rc::Rc;

/// Место повтора в кадре: узел раскладки, если дерево кадра его отдало.
pub(in crate::layout::reuse) struct ReplaySlot {
    pub(super) record: Rc<ReplayRecord>,
    pub(super) layout: Cell<Option<LayoutId>>,
    /// How far the block lies from where it painted (`valid`): a content
    /// record whose output moves (`Ranges::movable`) replays moved.
    pub(super) offset: Cell<Point<Pixels>>,
}

impl ReplaySlot {
    pub(in crate::layout::reuse) fn new(record: Rc<ReplayRecord>) -> Self {
        Self {
            record,
            layout: Cell::new(None),
            offset: Cell::new(Point::default()),
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
                _ if r.mask != mask => Some("mask"),
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
