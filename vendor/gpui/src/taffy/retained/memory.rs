//! KaminIDE patch: retained layout bookkeeping is a separate heap owner.

use super::{Candidates, Retained};
use crate::heap_report::{HeapSink, table, vec};
use std::{any::Any, rc::Rc};
use taffy::tree::NodeId;

impl Retained {
    pub(in crate::taffy) fn heap_report(&self, sink: &mut HeapSink) {
        let path = "gpui/retained-layout";
        vec(sink, path, &self.prev);
        vec(sink, path, &self.seq);
        table::<NodeId>(sink, path, self.live.capacity());
        table::<(NodeId, u64)>(sink, path, self.keys.capacity());
        table::<(u64, Candidates)>(sink, path, self.by_key.capacity());
        for candidates in self.by_key.values() {
            vec(sink, path, &candidates.positions);
        }
        // Host memo payloads are opaque and can be shared with the host. Only
        // this table's own allocation is reported here.
        table::<(NodeId, Rc<dyn Any>)>(sink, path, self.memos.capacity());
        table::<(NodeId, taffy::SharedStyle)>(sink, path, self.overrides.capacity());
    }
}
