//! Replay records over a recolouring restyle: a block whose subtree paints
//! no box in its colour (`dom::same_unread_color_tree_as`) keeps its recorded
//! layout, prepaint and paint when only `color` changed above it or in it
//! (Blink keeps the cached display items of objects a colour change does not
//! paint-invalidate, `PaintController::UseCachedItemIfPossible`,
//! paint_controller.cc:228; Gecko keeps the unchanged items of its retained
//! display list, `RetainedDisplayListBuilder`). The restyle does not compare
//! them: the next build does, by the block's source (`ReplayRecord::fits`),
//! and a record it does not replay goes at the end of that frame
//! (`FlowReuse::finish_replays`).

use super::Store;
use super::replay::ReplayRecord;
use crate::dom::Node;
use std::collections::HashMap;
use std::rc::Rc;

/// Replay records carried over a restyle.
pub(super) type Replays = Option<HashMap<u64, Rc<ReplayRecord>>>;

impl Store {
    /// The document `old` was rebuilt as `new` from the same source DOM: the
    /// block records are compared (`rebase_records`). Replay records go,
    /// unless the rebuild is a restyle of the known elements `restyled`,
    /// which keeps the source DOM and the node ids: then the next build
    /// compares them.
    pub(crate) fn rebase(&self, old: &[Node], new: &[Node], restyled: Option<&[u64]>) {
        let replays: Replays = restyled.map(|_| self.replays.take());
        self.rebase_records(old, new, restyled);
        if let Some(replays) = replays {
            self.replays.replace(replays);
        }
    }
}
