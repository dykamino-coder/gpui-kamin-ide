//! Удерживаемые блоки T3 по владельцам: слоты, синтетика и логические IFC.

use super::{BoxRecord, BoxTree, inline::InlineContent};
use kamin_heap::{HeapSize, Ops};
use std::rc::Rc;

impl HeapSize for BoxTree {
    fn heap_size(&self, ops: &mut Ops) -> usize {
        self.arena.heap_size(ops)
            + self.flows.heap_size(ops)
            + self.by_node.heap_size(ops)
            + self
                .finished
                .as_ref()
                .map_or(0, |(_, inputs)| inputs.flow_heap_size(ops))
            + ops.at("layout/used-styles", |_| self.used.heap_size())
    }
}

impl HeapSize for BoxRecord {
    fn heap_size(&self, ops: &mut Ops) -> usize {
        ops.at("document/box-sources", |ops| self.origin.heap_size(ops))
            + ops.at("text/box-ifc", |ops| {
                self.inline.heap_size(ops) + self.prepared.heap_size(ops)
            })
            + ops.at("layout/atom-styles", |ops| {
                (self.atom_style.as_ref()).map_or(0, |kept| {
                    ops.shallow(Rc::as_ptr(kept)) + kept.heap_size(ops)
                })
            })
    }
}

impl HeapSize for InlineContent {
    fn heap_size(&self, ops: &mut Ops) -> usize {
        self.text.heap_size(ops) + ops.buffer(self.items.as_ptr(), self.items.capacity())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::Node;
    use std::{cell::RefCell, rc::Rc};

    #[test]
    fn the_report_counts_retained_flows_once_and_releases_removed_content() {
        let tree = Rc::new(RefCell::new(BoxTree::default()));
        tree.borrow_mut()
            .publish(&[Node::Text("retained text".into())]);
        let mut ops = Ops::default();
        assert!(ops.at("document/box-arena", |ops| tree.heap_size(ops)) > 0);
        assert!(ops.report().entries()["text/box-ifc"] > 0);
        assert_eq!(tree.heap_size(&mut ops), 0, "shared arena is counted once");
        tree.borrow_mut().publish(&[]);
        let mut ops = Ops::default();
        tree.heap_size(&mut ops);
        assert!(!ops.report().entries().contains_key("text/box-ifc"));
        assert!(ops.report().total() > 0, "vacant slots remain arena-owned");
    }
}
