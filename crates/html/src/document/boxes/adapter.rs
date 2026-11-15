//! Rebind transient adapter slices when an unread attribute copies a path.

use super::{BoxTree, finish::slice_key, source::Source};
use crate::dom::{BoxId, Node};

impl BoxTree {
    /// Clean commits preserve topology and all inputs read by IFC preparation.
    /// Only the adapter allocation changes; neither slots nor prepared runs do.
    pub(crate) fn rebind(&mut self, nodes: &[Node]) {
        self.flows.clear();
        let root = self
            .get(self.first)
            .is_some_and(|record| record.origin == Source::Root);
        let first = if root {
            self.flows.insert(slice_key(nodes), self.first);
            self.get(self.first).map_or(BoxId::NONE, |r| r.first)
        } else {
            self.first
        };
        let mut pending = vec![(nodes.iter(), first)];
        while let Some((siblings, next)) = pending.last_mut() {
            match siblings.next() {
                Some(Node::Element(e)) => {
                    let id = *next;
                    let Some(record) = self.get(id) else { continue };
                    let (first, sibling) = (record.first, record.next);
                    *next = sibling;
                    if !e.children.is_empty() {
                        self.flows.insert(slice_key(&e.children), id);
                    }
                    pending.push((e.children.iter(), first));
                }
                Some(Node::Text(_)) => {}
                None => {
                    pending.pop();
                }
            }
        }
    }
}
