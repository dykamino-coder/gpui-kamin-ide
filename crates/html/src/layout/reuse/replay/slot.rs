//! Место повтора блока в кадре и его проверка перед prepaint.

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

    /// Повтор этого кадра точен: узлы заняты, ни одна мера не понадобилась,
    /// блок лёг на записанное место того же размера в том же окружении, а
    /// диапазоны записаны в только что показанном кадре.
    pub(in crate::layout::reuse) fn valid(&self, window: &mut Window) -> bool {
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
        // A box's content (`content`, no flow key) or an inert root-flow
        // block may lie elsewhere: its movable output replays moved.
        let content = self.record.key.is_none();
        let moves = content || self.record.inert;
        let placed = self.record.ranges(|r| {
            // Its leaves keep their functions: one measured again may lie or
            // draw otherwise (`Window::retained_remeasured`).
            let remeasured = content && window.retained_remeasured(&r.layout);
            let moved = r.origin != origin;
            let fits = !remeasured
                && r.serial + 1 == serial
                && r.lines.complete()
                && r.scale == scale
                && r.mask == mask
                && r.size == size
                && (!moved || moves && r.movable);
            (fits, origin - r.origin)
        });
        match placed {
            Some((true, offset)) if fresh => {
                self.offset.set(offset);
                true
            }
            _ => false,
        }
    }
}
