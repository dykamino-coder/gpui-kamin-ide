//! Normalize text-only IFC after materialization, before GPUI build.

#[cfg(test)]
mod tests;

use super::{
    BoxTree,
    inline::{InlineContent, Item, ItemKind},
};
use crate::dom::{BoxId, Node};
use crate::style::computed::{BoxStyle, Computed};
use crate::text::inline::normalization::{Inputs, Normalizer};
use std::rc::Rc;

#[derive(Debug)]
pub(super) struct Prepared {
    source: Rc<InlineContent>,
    inputs: Inputs,
    pub(super) text: Rc<InlineContent>,
}

pub(super) fn slice_key(nodes: &[Node]) -> (usize, usize) {
    (nodes.as_ptr() as usize, nodes.len())
}

impl BoxTree {
    /// Addresses only select records of this published adapter; no pointer is dereferenced.
    pub(crate) fn finish(&mut self, nodes: &[Node], root: &BoxStyle, changed: bool) {
        if !changed
            && self
                .finished
                .as_ref()
                .is_some_and(|(epoch, old)| *epoch == self.epoch && old.matches(root))
        {
            return;
        }
        self.finished = Some((self.epoch, Inputs::of(root)));
        self.finish_flow(nodes, root);
        let mut pending = vec![nodes.iter()];
        while let Some(siblings) = pending.last_mut() {
            match siblings.next() {
                Some(Node::Element(e)) => {
                    self.finish_flow(&e.children, &e.style.used());
                    pending.push(e.children.iter());
                }
                Some(Node::Text(_)) => {}
                None => {
                    pending.pop();
                }
            }
        }
    }

    fn finish_flow(&mut self, nodes: &[Node], style: &Computed) {
        let Some(&id) = self.flows.get(&slice_key(nodes)) else {
            return;
        };
        if nodes.iter().any(|node| matches!(node, Node::Element(_))) {
            self.arena.prepared(id, None);
            return;
        }
        self.prepare(id, style);
    }

    /// A synthetic or changed layout parent must use its own normalization inputs.
    pub(crate) fn normalized(&self, nodes: &[Node], style: &Computed) -> Option<Rc<InlineContent>> {
        self.normalized_inline(*self.flows.get(&slice_key(nodes))?, style)
    }

    /// Stable T3 input for layout: offsets address this UTF-8 buffer and retain leaf BoxIds.
    pub(crate) fn normalized_inline(
        &self,
        id: BoxId,
        style: &Computed,
    ) -> Option<Rc<InlineContent>> {
        let record = self.get(id)?;
        let prepared = record.prepared.as_ref()?;
        (prepared.inputs.matches(style)).then(|| prepared.text.clone())
    }
}

impl BoxTree {
    pub(super) fn prepare(&mut self, id: BoxId, style: &Computed) {
        let Some(record) = self.get(id) else { return };
        let Some(source) = record.inline.as_ref() else {
            self.arena.prepared(id, None);
            return;
        };
        // Mixed flows need inline-box paint/edge/atom lowering, handled by the adapter.
        if !source
            .items
            .iter()
            .all(|item| matches!(item.kind, ItemKind::Text { .. }))
        {
            self.arena.prepared(id, None);
            return;
        }
        let inputs = Inputs::of(style);
        if record
            .prepared
            .as_ref()
            .is_some_and(|p| Rc::ptr_eq(&p.source, source) && p.inputs == inputs)
        {
            return;
        }
        let mut normalizer = Normalizer::default();
        let mut text = InlineContent::default();
        for item in &source.items {
            let ItemKind::Text { start, end } = item.kind else {
                continue;
            };
            let run = normalizer.text(&source.text[start..end], style);
            if run.is_empty() {
                continue;
            }
            let start = text.text.len();
            text.text.push_str(&run);
            text.items.push(Item {
                box_id: item.box_id,
                kind: ItemKind::Text {
                    start,
                    end: text.text.len(),
                },
            });
        }
        let prepared = Prepared {
            source: source.clone(),
            inputs,
            text: Rc::new(text),
        };
        self.arena.prepared(id, Some(prepared));
    }
}

#[cfg(feature = "heap-report")]
impl kamin_heap::HeapSize for Prepared {
    fn heap_size(&self, ops: &mut kamin_heap::Ops) -> usize {
        self.source.heap_size(ops) + self.inputs.flow_heap_size(ops) + self.text.heap_size(ops)
    }
}
