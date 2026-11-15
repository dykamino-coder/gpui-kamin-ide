//! Коробка потока по адресу среза входа, которым он опубликован.
//!
//! Адрес выбирает запись, но не разыменовывается. Освобождённый срез может
//! вернуть свой адрес аллокатору и новому срезу, поэтому запись снимается,
//! когда её коробка удалена или опубликована с другим срезом: иначе новый
//! поток получил бы чужую коробку. Коробка помнит свой адрес (`BoxRecord::flow`).

use super::BoxTree;
use crate::dom::{BoxId, Node};
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct Flows(HashMap<usize, (usize, BoxId)>);

fn address(nodes: &[Node]) -> usize {
    nodes.as_ptr() as usize
}

impl Flows {
    pub(super) fn get(&self, nodes: &[Node]) -> Option<BoxId> {
        let &(len, id) = self.0.get(&address(nodes))?;
        (len == nodes.len()).then_some(id)
    }

    pub(super) fn clear(&mut self) {
        self.0.clear();
    }

    #[cfg(feature = "heap-report")]
    pub(super) fn capacity(&self) -> usize {
        self.0.capacity()
    }

    /// Снять запись `at`, если она всё ещё за коробкой `id`.
    pub(super) fn forget(&mut self, at: usize, id: BoxId) {
        if at != 0 && self.0.get(&at).is_some_and(|&(_, owner)| owner == id) {
            self.0.remove(&at);
        }
    }
}

impl BoxTree {
    /// Коробка `id` публикует поток срезом `nodes` (пустой срез — без потока).
    pub(super) fn set_flow(&mut self, nodes: &[Node], id: BoxId) {
        let at = if nodes.is_empty() { 0 } else { address(nodes) };
        let Some(record) = self.arena.record_mut(id) else {
            return;
        };
        let old = std::mem::replace(&mut record.flow, at);
        if old != at {
            self.flows.forget(old, id);
        }
        if at != 0 {
            self.flows.0.insert(at, (nodes.len(), id));
        }
    }

    /// Коробка потока, опубликованного срезом `nodes`.
    #[cfg(test)]
    pub(crate) fn flow(&self, nodes: &[Node]) -> Option<BoxId> {
        self.flows.get(nodes)
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
        // A freed slice gives its address to the allocator: the old address
        // must not select the box any more.
        assert_eq!(boxes.flow(paragraph(&old)), None);
    }
}
