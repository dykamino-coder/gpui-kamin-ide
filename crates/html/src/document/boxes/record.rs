//! Allocation and reuse of a published box slot.

use super::{BoxRecord, BoxTree, source::Source};
use crate::document::delta::{BoxDelta, Damage};
use crate::dom::BoxId;

impl BoxTree {
    /// The slot of a source: the previous `reused` or a new one. A new box or one moved
    /// to another parent is marked changed.
    pub(super) fn record(
        &mut self,
        origin: Source,
        parent: BoxId,
        source: u64,
        reused: Option<BoxId>,
        changed: &mut BoxDelta,
    ) -> BoxId {
        if let Some(id) = reused
            && let Some(record) = self.arena.record_mut(id)
        {
            if record.parent != parent {
                changed.mark(id, Damage::STRUCTURE);
            }
            record.parent = parent;
            record.source = source;
            record.next = BoxId::NONE;
            return id;
        }
        let id = self.arena.insert(BoxRecord {
            parent,
            first: BoxId::NONE,
            next: BoxId::NONE,
            source,
            origin,
            inline: None,
            prepared: None,
            epochs: Default::default(),
            external: false,
            distributes: false,
            layout_slot: None,
        });
        let Some(id) = id else { return BoxId::NONE };
        changed.mark(id, Damage::STRUCTURE);
        id
    }
}
