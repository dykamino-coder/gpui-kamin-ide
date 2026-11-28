//! The box of a flow by its nodes, without the slice address in the key.
//!
//! The candidate is the owner of the first keyed node: the parent of the element's box
//! (`by_node`) or the owner of the source text the flow's text starts with
//! (`Text::first_source`, `Flows`).
//! The confirmation is the mark of the children snapshot the box published the flow with
//! (`ChildrenMark`): the mark holds the snapshot, and a live slice is compared only with
//! a live snapshot, so a freed and reallocated slice of another
//! flow does not pick the box. The document's root flow (not a children snapshot)
//! is confirmed by the keys of all its nodes and their count.

use super::BoxTree;
use super::by_node::ByNode;
use super::source::Source;
use crate::dom::{BoxId, Children, ChildrenMark, Node};

/// The owner of a flow by the source text its text starts with, and the
/// marks of the published flows by box slot.
#[derive(Default)]
pub(super) struct Flows {
    texts: ByNode,
    /// The children snapshot the box of the slot (of its generation) published its flow with.
    marks: Vec<Option<(u32, ChildrenMark)>>,
    /// The node count of the root flow of the `Source::Root` box.
    root: usize,
}

impl Flows {
    pub(super) fn clear(&mut self) {
        self.texts.clear();
        self.marks.clear();
        self.root = 0;
    }

    fn set_mark(&mut self, id: BoxId, mark: Option<ChildrenMark>) {
        let slot = id.slot as usize;
        if slot >= self.marks.len() {
            if mark.is_none() {
                return;
            }
            self.marks.resize_with(slot + 1, || None);
        }
        self.marks[slot] = mark.map(|mark| (id.generation, mark));
    }

    pub(super) fn rebind_mark(&mut self, id: BoxId, children: &Children) {
        if !self.mark(id).is_some_and(|mark| mark.holds(children)) {
            self.set_mark(id, Some(children.mark()));
        }
    }

    fn mark(&self, id: BoxId) -> Option<&ChildrenMark> {
        match self.marks.get(id.slot as usize)? {
            Some((generation, mark)) if *generation == id.generation => Some(mark),
            _ => None,
        }
    }

    /// Box `id` is removed: its mark does not hold a snapshot.
    pub(super) fn forget(&mut self, id: BoxId) {
        self.set_mark(id, None);
    }

    #[cfg(feature = "heap-report")]
    pub(super) fn heap_size(&self, ops: &mut kamin_heap::Ops) -> usize {
        self.texts.heap_size(ops) + self.marks.capacity() * size_of::<Option<(u32, ChildrenMark)>>()
    }

    fn own_texts(&mut self, nodes: &[Node], id: BoxId) {
        for node in nodes {
            if let Node::Text(text) = node
                && let Some(source) = text.first_source()
            {
                self.texts.set(u64::from(source.get()), id);
            }
        }
    }
}

impl BoxTree {
    /// Box `id` publishes a flow with children `children` (empty: no flow).
    pub(super) fn set_flow(&mut self, children: &Children, id: BoxId) {
        if self.get(id).is_none() {
            return;
        }
        let mark = (!children.is_empty()).then(|| children.mark());
        self.flows.set_mark(id, mark);
        self.flows.own_texts(children, id);
    }

    /// Root box `root` publishes the document flow `nodes`.
    pub(super) fn set_root_flow(&mut self, nodes: &[Node], root: BoxId) {
        self.flows.set_mark(root, None);
        self.flows.root = nodes.len();
        self.flows.own_texts(nodes, root);
    }

    /// The owner of a keyed node: the parent box of an element or the owner of a text.
    fn owner(&self, node: &Node) -> Option<Option<BoxId>> {
        match node {
            Node::Element(e) if e.node_id != 0 => Some(
                self.by_node
                    .get(e.node_id)
                    .and_then(|id| self.get(id))
                    .map(|record| record.parent),
            ),
            Node::Text(text) => text
                .first_source()
                .map(|source| self.flows.texts.get(u64::from(source.get()))),
            Node::Element(_) => None,
        }
    }

    /// The box of the flow published by nodes `nodes`. A candidate is the owner of a node:
    /// the nodes of an anonymous run name the run's box (its texts) and the parent of
    /// their elements, so each distinct one is tried; the mark of its snapshot confirms
    /// the whole slice.
    pub(crate) fn flow_of(&self, nodes: &[Node]) -> Option<BoxId> {
        let mut tried = None;
        nodes.iter().find_map(|node| {
            let owner = self.owner(node).flatten()?;
            (tried.replace(owner) != Some(owner) && self.publishes(owner, nodes)).then_some(owner)
        })
    }

    /// Box `owner` published `nodes` as its flow (`flow_of`).
    fn publishes(&self, owner: BoxId, nodes: &[Node]) -> bool {
        let Some(record) = self.get(owner) else {
            return false;
        };
        match self.flows.mark(owner) {
            Some(mark) => mark.holds(nodes),
            None => {
                record.origin == Source::Root
                    && self.flows.root == nodes.len()
                    && nodes
                        .iter()
                        .all(|node| self.owner(node).is_none_or(|o| o == Some(owner)))
            }
        }
    }

    /// The box of the flow published by slice `nodes`.
    #[cfg(test)]
    pub(crate) fn flow(&self, nodes: &[Node]) -> Option<BoxId> {
        self.flow_of(nodes)
    }
}

#[cfg(test)]
mod tests {
    use crate::document::Document;
    use crate::dom::Node;

    fn paragraph(nodes: &[Node]) -> &[Node] {
        let mut pending = vec![nodes.iter()];
        while let Some(siblings) = pending.last_mut() {
            match siblings.next() {
                Some(Node::Element(e)) if e.tag == "p" => return &e.children,
                Some(Node::Element(e)) => pending.push(e.children.iter()),
                Some(Node::Text(_)) => {}
                None => {
                    pending.pop();
                }
            }
        }
        panic!("no paragraph")
    }

    #[test]
    fn a_republished_flow_forgets_the_address_of_its_old_slice() {
        let mut doc = Document::builder("<div><p>one</p></div>", "")
            .build()
            .unwrap();
        let old = doc.nodes.clone();
        let id = doc
            .state
            .boxes
            .borrow()
            .flow(paragraph(&old))
            .expect("flow");
        doc.update("<div><p>two</p></div>", "").unwrap();
        let boxes = doc.state.boxes.borrow();
        assert_eq!(
            boxes.flow(paragraph(&doc.nodes)),
            Some(id),
            "the box is kept"
        );
        // The old slice is not the published one any more, though its text
        // has the same source.
        assert_eq!(boxes.flow(paragraph(&old)), None);
    }

    #[test]
    fn a_copied_slice_with_the_same_sources_is_not_the_published_flow() {
        let doc = Document::builder("<div><p>one</p></div>", "")
            .build()
            .unwrap();
        let published = paragraph(&doc.nodes);
        let copy: Vec<Node> = published.to_vec();
        let boxes = doc.state.boxes.borrow();
        assert!(boxes.flow(published).is_some());
        assert_eq!(boxes.flow(&copy), None);
    }
}
