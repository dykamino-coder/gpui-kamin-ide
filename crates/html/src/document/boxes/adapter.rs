//! Rebind transient adapter slices when an unread attribute copies a path.

use super::{BoxTree, source::Source};
use crate::dom::{BoxId, Node};

impl BoxTree {
    /// Raw text is distinct from CSS Text normalization (direction, shadows, hashing).
    pub(crate) fn raw_text(&self, nodes: &[Node]) -> Option<super::inline::InlineContent> {
        let record = self.get(self.flow_of(nodes)?)?;
        let text = record.inline.as_ref()?;
        text.items()
            .iter()
            .all(|item| matches!(item.kind, super::inline::ItemKind::Text { .. }))
            .then(|| text.clone())
    }

    /// The retained text of the flow of `nodes`, normalized when it was
    /// prepared: a paragraph whose text equals it takes it as its own
    /// (`InlineContent::shared_text`) instead of a copy.
    pub(crate) fn flow_text(&self, nodes: &[Node]) -> Option<gpui::SharedString> {
        let record = self.get(self.flow_of(nodes)?)?;
        let prepared = record.prepared.as_ref().and_then(|p| p.normalized());
        prepared.or(record.inline.as_ref()).map(|t| t.shared_text())
    }

    /// Clean commits preserve topology and all inputs read by IFC preparation.
    /// Only the adapter allocation changes; neither slots nor prepared runs do.
    pub(crate) fn rebind(&mut self, nodes: &[Node]) {
        self.flows.clear();
        let root = self
            .get(self.first)
            .is_some_and(|record| record.origin == Source::Root);
        let first = if root {
            self.set_root_flow(nodes, self.first);
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
                    self.set_flow(&e.children, id);
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
