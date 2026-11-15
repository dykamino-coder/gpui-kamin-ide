//! Память индексов исходного DOM; сами узлы меряет владелец исходного дерева.

use super::TraversalState;
use kamin_heap::{HeapSize, Ops};

impl HeapSize for TraversalState {
    fn heap_size(&self, ops: &mut Ops) -> usize {
        if !ops.once(self) {
            return 0;
        }
        let keys = self.text_keys.borrow();
        self.elements.heap_size(ops)
            + self.texts.heap_size(ops)
            + ops.buffer(keys.as_ptr(), keys.capacity())
            + self.read.heap_size(ops)
    }
}
