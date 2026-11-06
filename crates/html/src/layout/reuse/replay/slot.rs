//! Место повтора блока в кадре и его проверка перед prepaint.

use super::{ReplayRecord, plain_environment};
use gpui::{LayoutId, Window};
use std::cell::Cell;
use std::rc::Rc;

/// Место повтора в кадре: узел раскладки, если дерево кадра его отдало.
pub(in crate::layout::reuse) struct ReplaySlot {
    pub(super) record: Rc<ReplayRecord>,
    pub(super) layout: Cell<Option<LayoutId>>,
}

impl ReplaySlot {
    pub(in crate::layout::reuse) fn new(record: Rc<ReplayRecord>) -> Self {
        Self {
            record,
            layout: Cell::new(None),
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
        fresh
            && self
                .record
                .ranges(|r| {
                    r.serial + 1 == serial
                        && r.lines.complete()
                        && r.scale == scale
                        && r.mask == mask
                        && r.origin == origin
                        && r.size == size
                })
                .unwrap_or(false)
    }
}
