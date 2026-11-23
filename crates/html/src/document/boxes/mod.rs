//! The document's box tree: stable slots and the mutable links between them.
//!
//! The input Vec<Node> still serves as the preparation adapter. The arena keeps no snapshots of it:
//! a publication changes the existing records, and a removal bumps the slot generation.

mod adapter;
mod arena;
#[cfg(test)]
mod box_of_tests;
mod by_node;
mod claims;
#[cfg(test)]
mod delta_tests;
mod epochs;
mod record;
mod reservations;
pub(crate) use epochs::Epochs;
#[cfg(test)]
mod epochs_tests;
mod finish;
mod flows;
pub(crate) use finish::MixedText;
#[cfg(feature = "heap-report")]
mod heap;
pub(crate) mod inline;
#[cfg(test)]
mod key_tests;
mod level;
mod links;
#[cfg(test)]
mod move_tests;
mod publish;
mod reuse;
mod source;
#[cfg(test)]
mod source_tests;
mod sources;
#[cfg(test)]
mod tests;
pub(crate) mod used;

use crate::dom::{BoxId, Node};
use arena::Arena;
use source::Source;

pub(crate) struct BoxRecord {
    pub(crate) parent: BoxId,
    pub(crate) first: BoxId,
    pub(crate) next: BoxId,
    source: u64,
    origin: Source,
    pub(crate) inline: Option<inline::InlineContent>,
    prepared: Option<Box<finish::Prepared>>,
    /// Publication numbers at which the layout inputs changed.
    pub(crate) epochs: Epochs,
    /// The contribution to the ancestor's intrinsic sizes does not depend on the content.
    external: bool,
    /// The box sizes its children from all their intrinsic contributions
    /// (table, flex, grid): a child's width follows its siblings' content.
    distributes: bool,
    /// The retained layout result of the box; layout owns it.
    layout_slot: Option<Box<crate::layout::retained::LayoutSlot>>,
}

#[derive(Default)]
pub(crate) struct BoxTree {
    arena: Arena,
    flows: flows::Flows,
    /// The first box of each keyed source element (`Element::node_id`):
    /// layout finds its box in O(1); fragment copies share it.
    by_node: by_node::ByNode,
    first: BoxId,
    epoch: u64,
    /// Publication number for the layout epochs (`Epochs`).
    clock: u32,
    finished: Option<(u64, crate::text::inline::normalization::Inputs)>,
    /// The flows the last publication walked, until a frame prepares them.
    visited: finish::Visited,
    /// Used styles derived from computed ones, shared by the boxes of one style.
    pub(crate) used: used::UsedStyles,
}

impl BoxTree {
    pub(crate) fn is_empty(&self) -> bool {
        !self.first.is_some()
    }

    /// The one mutable entry to a document's box tree. Used styles in it are
    /// shared values (`used`): no caller reaches a `Computed` through it.
    pub(crate) fn edit(tree: &std::cell::RefCell<Self>) -> std::cell::RefMut<'_, Self> {
        tree.borrow_mut()
    }

    /// The box of the source element with key `node_id` (its first fragment).
    pub(crate) fn box_of(&self, node_id: u64) -> Option<BoxId> {
        self.by_node.get(node_id)
    }

    pub(crate) fn get(&self, id: BoxId) -> Option<&BoxRecord> {
        self.arena.get(id)
    }

    /// Live boxes without walking the adapter tree Element/Children.
    /// The slot order is not the document order: first/next give it.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (BoxId, &BoxRecord)> {
        self.arena.iter()
    }

    /// Publish an accepted input of the same shape.
    /// The source is given by the DOM NodeKey, the synthetic role apart from it.
    #[cfg(test)]
    fn publish(&mut self, nodes: &[Node]) {
        self.publish_sources(nodes);
    }

    /// `previous`: the input of the previous publication, still alive: the layout epochs
    /// are set by it; without it every box counts as changed.
    pub(crate) fn publish_document(
        &mut self,
        nodes: &[Node],
        previous: Option<&[Node]>,
        _values: &crate::values::DocumentValues,
    ) -> super::delta::BoxDelta {
        let element = |id| Some(crate::dom::NodeKey::from_packed(id));
        let keys = publish::Keys { element: &element };
        self.publish_keyed(nodes, previous, &keys)
    }

    #[cfg(test)]
    fn publish_sources(&mut self, nodes: &[Node]) {
        let element = |id| Some(crate::dom::NodeKey::from_packed(id));
        let keys = publish::Keys { element: &element };
        self.publish_keyed(nodes, None, &keys);
    }

    /// A full change of shape before persistent DOM keys exist: the old box keys
    /// are invalidated, and the slots get new generations.
    pub(crate) fn replace(&mut self, nodes: &[Node], values: &crate::values::DocumentValues) {
        let ids: Vec<_> = self.iter().map(|(id, _)| id).collect();
        for id in ids {
            self.arena.remove(id);
        }
        self.first = BoxId::NONE;
        self.by_node.clear();
        self.flows.clear();
        self.publish_document(nodes, None, values);
    }
}
