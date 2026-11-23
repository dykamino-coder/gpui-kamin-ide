//! Account the single flat map and shared direct segments once.
use super::*;
use kamin_heap::{HeapSize, Ops};

impl HeapSize for BindingStore {
    fn heap_size(&self, ops: &mut Ops) -> usize {
        if !ops.once(self) {
            return 0;
        }
        self.nodes.capacity() * std::mem::size_of::<(NodeKey, Counts)>()
            + self.segments.capacity() * std::mem::size_of::<(NodeKey, Rc<BindingSegment>)>()
            + self
                .segments
                .values()
                .filter(|segment| ops.once(Rc::as_ptr(segment)))
                .map(|segment| {
                    std::mem::size_of::<BindingSegment>()
                        + std::mem::size_of_val(segment.direct.as_ref())
                })
                .sum::<usize>()
    }
}

impl HeapSize for BindingRecorder {
    fn heap_size(&self, ops: &mut Ops) -> usize {
        if !ops.once(self) {
            return 0;
        }
        let drafts = self.stack.iter().chain(std::iter::once(&self.root));
        self.stack.capacity() * std::mem::size_of::<super::recording::Draft>()
            + drafts
                .map(|draft| draft.direct.capacity() * std::mem::size_of::<(NodeKey, Binding)>())
                .sum::<usize>()
            + self.text_addresses.capacity() * std::mem::size_of::<(TextId, usize)>()
            + self.changed.capacity() * std::mem::size_of::<(NodeKey, Rc<BindingSegment>)>()
            + self
                .changed
                .values()
                .filter(|segment| ops.once(Rc::as_ptr(segment)))
                .map(|segment| {
                    std::mem::size_of::<BindingSegment>()
                        + std::mem::size_of_val(segment.direct.as_ref())
                })
                .sum::<usize>()
    }
}
