//! Publish replacement flow snapshots when a revision keeps the same boxes.

use super::{BoxTree, source::Source};
use crate::dom::Node;

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
    /// (`InlineContent::shared_text`) instead of a copy. A slice the box tree
    /// did not publish (a render copy of the children) takes the text of the
    /// box of `container`, the element whose content `nodes` are: the caller
    /// shares it only when it is equal.
    pub(crate) fn flow_text(&self, nodes: &[Node], container: u64) -> Option<gpui::SharedString> {
        let record = self.get(self.flow_of(nodes).or_else(|| self.box_of(container))?)?;
        let prepared = record.prepared.as_ref().and_then(|p| p.normalized());
        prepared.or(record.inline.as_ref()).map(|t| t.shared_text())
    }

    /// Attribute and paint writes preserve topology and text sources. Only
    /// ancestor lists copied by COW acquire a new published snapshot.
    pub(crate) fn rebind_path(
        &mut self,
        mut nodes: &[Node],
        path: &[usize],
        unique: impl Fn(u64) -> bool,
    ) {
        let Some((_, parents)) = path.split_last() else {
            return;
        };
        let mut first = self.first;
        if let Some(root) = self.get(first)
            && root.origin == Source::Root
        {
            first = root.first;
        }
        for &index in parents {
            let Some(Node::Element(e)) = nodes.get(index) else {
                return;
            };
            // Anonymous boxes and copies of a source are addressed by their
            // exact sibling occurrence. Text nodes occupy no arena slot.
            let id = if unique(e.node_id) {
                self.box_of(e.node_id)
            } else {
                nodes[..index]
                    .iter()
                    .filter(|n| matches!(n, Node::Element(_)))
                    .try_fold(first, |id, _| Some(self.get(id)?.next))
            };
            let Some(id) = id else { return };
            let Some(record) = self.get(id) else { return };
            first = record.first;
            self.flows.rebind_mark(id, &e.children);
            nodes = &e.children;
        }
    }
}
