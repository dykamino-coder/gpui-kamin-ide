//! Memory of the source bindings; source DOM owns its nodes.
use super::TraversalState;
use kamin_heap::{HeapSize, Ops};
impl HeapSize for TraversalState {
    fn heap_size(&self, ops: &mut Ops) -> usize {
        if !ops.once(self) {
            return 0;
        }
        self.binding_owner().borrow().heap_size(ops) + self.recording.borrow().heap_size(ops)
    }
}
