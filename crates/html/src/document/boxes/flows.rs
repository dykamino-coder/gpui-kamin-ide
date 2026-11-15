//! Коробка потока по его узлам, без адреса среза в ключе.
//!
//! Кандидат — владелец первого узла с ключом: родитель коробки элемента
//! (`by_node`) или владелец исходного текста, с которого начинается текст
//! (`Text::first_source`, `Flows`).
//! Подтверждение — метка снимка детей, которым коробка опубликовала поток
//! (`ChildrenMark`): метка держит снимок, и живой срез сравнивается только с
//! живым снимком, так что освобождённый и заново выделенный срез чужого
//! потока не выбирает коробку. Корневой поток документа (не снимок детей)
//! подтверждается ключами всех своих узлов и длиной.

use super::BoxTree;
use super::by_node::ByNode;
use super::source::Source;
use crate::dom::{BoxId, Children, Node};

/// Владелец потока по исходному тексту, с которого начинается его текст.
#[derive(Default)]
pub(super) struct Flows {
    texts: ByNode,
    /// Число узлов корневого потока коробки `Source::Root`.
    root: usize,
}

impl Flows {
    pub(super) fn clear(&mut self) {
        self.texts.clear();
        self.root = 0;
    }

    #[cfg(feature = "heap-report")]
    pub(super) fn heap_size(&self, ops: &mut kamin_heap::Ops) -> usize {
        self.texts.heap_size(ops)
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
    /// Коробка `id` публикует поток детьми `children` (пустые — без потока).
    pub(super) fn set_flow(&mut self, children: &Children, id: BoxId) {
        let Some(record) = self.arena.record_mut(id) else {
            return;
        };
        record.flow = (!children.is_empty()).then(|| children.mark());
        self.flows.own_texts(children, id);
    }

    /// Корневая коробка `root` публикует поток документа `nodes`.
    pub(super) fn set_root_flow(&mut self, nodes: &[Node], root: BoxId) {
        if let Some(record) = self.arena.record_mut(root) {
            record.flow = None;
        }
        self.flows.root = nodes.len();
        self.flows.own_texts(nodes, root);
    }

    /// Владелец узла с ключом: коробка-родитель элемента или владелец текста.
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

    /// Коробка потока, опубликованного узлами `nodes`.
    pub(crate) fn flow_of(&self, nodes: &[Node]) -> Option<BoxId> {
        let owner = nodes.iter().find_map(|node| self.owner(node))??;
        let record = self.get(owner)?;
        let published = match &record.flow {
            Some(mark) => mark.holds(nodes),
            None => {
                record.origin == Source::Root
                    && self.flows.root == nodes.len()
                    && nodes
                        .iter()
                        .all(|node| self.owner(node).is_none_or(|o| o == Some(owner)))
            }
        };
        published.then_some(owner)
    }

    /// Коробка потока, опубликованного срезом `nodes`.
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
