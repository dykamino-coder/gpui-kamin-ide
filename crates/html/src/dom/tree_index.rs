//! Side tables of one document parse, keyed by DOM node: declarative shadow
//! roots, slot assignments, `::slotted` rules and `:has()` marks — plus the
//! document's state the walk reads and writes.
//!
//! html5ever's `RcDom` has no per-node rare data (where Blink keeps an
//! element's ShadowRoot), so the parse builds these tables before the style
//! walk and hands them to it explicitly. A key holds its `Handle`: the node
//! cannot be freed — and its address reused by another node — while the table
//! that names it is alive, and the table dies with the parse.

use crate::dom::shadow::{Shadow, SlotInfo};
use crate::style::css::Rule;
use markup5ever_rcdom::Handle;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

/// Identity of a DOM node: pointer equality on a kept-alive `Handle`.
#[derive(Clone)]
pub(crate) struct NodeKey(Handle);

impl NodeKey {
    pub(crate) fn of(handle: &Handle) -> Self {
        Self(handle.clone())
    }
}

impl PartialEq for NodeKey {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for NodeKey {}

impl Hash for NodeKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::ptr::hash(Rc::as_ptr(&self.0), state);
    }
}

/// Everything the style walk reads about a node besides the node itself.
#[derive(Default)]
pub(crate) struct TreeIndex {
    /// `:has()` marks: hashes of the arguments this node anchors a match for.
    pub(crate) has_marks: HashMap<NodeKey, Vec<u64>>,
    /// Shadow tree of a host.
    pub(crate) shadows: HashMap<NodeKey, Rc<Shadow>>,
    /// Assignment of a `<slot>`.
    pub(crate) slots: HashMap<NodeKey, Rc<SlotInfo>>,
    /// `::slotted()` rules that match a flattened assigned element.
    pub(crate) slotted: HashMap<NodeKey, Rc<Vec<Rule>>>,
    /// The document being parsed: its quirks mode, `@property` registry and
    /// root metrics, which the walk reads and writes.
    pub(crate) doc: Rc<crate::document::state::DocState>,
}

impl TreeIndex {
    pub(crate) fn new(doc: Rc<crate::document::state::DocState>) -> Self {
        TreeIndex {
            doc,
            ..Default::default()
        }
    }

    pub(crate) fn has_marks_of(&self, handle: &Handle) -> Vec<u64> {
        self.has_marks
            .get(&NodeKey::of(handle))
            .cloned()
            .unwrap_or_default()
    }

    pub(crate) fn shadow_of(&self, handle: &Handle) -> Option<Rc<Shadow>> {
        self.shadows.get(&NodeKey::of(handle)).cloned()
    }

    pub(crate) fn slot_of(&self, handle: &Handle) -> Option<Rc<SlotInfo>> {
        self.slots.get(&NodeKey::of(handle)).cloned()
    }

    pub(crate) fn slotted_rules_of(&self, handle: &Handle) -> Rc<Vec<Rule>> {
        self.slotted
            .get(&NodeKey::of(handle))
            .cloned()
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_compare_node_identity_not_content() {
        let dom = crate::dom::input::html("<p></p><p></p>");
        let body = |h: &Handle| -> Vec<Handle> { h.children.borrow().clone() };
        let html = body(&dom.document).pop().unwrap();
        let ps = body(&body(&html)[1]);
        assert!(NodeKey::of(&ps[0]) == NodeKey::of(&ps[0]));
        assert!(NodeKey::of(&ps[0]) != NodeKey::of(&ps[1]));
    }
}
