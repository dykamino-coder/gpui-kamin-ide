//! Память общего стиля узла по владельцам (`heap-report`, docs/memprof.md).

use super::SharedStyle;
use kamin_heap::{HeapSize, Ops};
use std::rc::Rc;

impl HeapSize for SharedStyle {
    fn heap_size(&self, ops: &mut Ops) -> usize {
        let facts = ops.at("style/layout-facts", |ops| {
            self.box_facts
                .as_ref()
                .map_or(0, |facts| ops.shallow(Rc::as_ptr(facts)))
                + self
                    .decoration
                    .as_ref()
                    .map_or(0, |input| ops.shallow(Rc::as_ptr(input)))
        });
        self.cascaded.heap_size(ops)
            + self.computed.heap_size(ops)
            + self.used.heap_size(ops)
            + facts
    }
}
