//! Source bindings have one committed owner; stages contain only replacements.
//!
//! A segment lists its own reads and source nodes, never descendant bindings.
//! Output ownership and retirement are supplied by the common memo publisher.

#[cfg(feature = "heap-report")]
mod heap;
mod recording;
mod stage;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod transaction_tests;

use crate::dom::TextId;
use markup5ever_rcdom::NodeKey;
pub use recording::{BindingRecorder, BindingRecording};
pub use stage::{BindingPublication, BindingStage, BindingUpdate};
use std::{collections::HashMap, rc::Rc};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Binding {
    address: usize,
    flags: u8,
}

const ELEMENT: u8 = 1;
const TEXT: u8 = 2;
const READ: u8 = 4;

/// A capability for one source owner's direct bindings. It owns no source DOM.
pub struct BindingSegment {
    owner: NodeKey,
    direct: Box<[(NodeKey, Binding)]>,
    _lease: Option<crate::budget::LiveReservation>,
}

impl BindingSegment {
    pub fn owner(&self) -> NodeKey {
        self.owner
    }
}

#[derive(Clone, Copy, Default, Debug, Eq, PartialEq)]
struct Counts {
    address: usize,
    flags: [usize; 3],
}

/// The source identity map of the current revision, independent of ValueCx.
#[derive(Default)]
pub struct BindingStore {
    identity: Rc<()>,
    generation: u64,
    segments: HashMap<NodeKey, Rc<BindingSegment>>,
    nodes: HashMap<NodeKey, Counts>,
}

impl BindingStore {
    pub fn stage(&self) -> BindingStage<'_> {
        BindingStage::new(self)
    }

    pub fn segment(&self, owner: NodeKey) -> Option<Rc<BindingSegment>> {
        self.segments.get(&owner).cloned()
    }

    pub fn id(&self, handle: &markup5ever_rcdom::Handle) -> Option<u64> {
        self.element(handle.key(), Rc::as_ptr(handle) as usize)
            .then_some(handle.key().packed())
    }

    pub fn element(&self, key: NodeKey, address: usize) -> bool {
        self.nodes
            .get(&key)
            .is_some_and(|n| n.address == address && n.flags[0] != 0)
    }

    pub fn text(&self, id: TextId) -> Option<usize> {
        let n = self.nodes.get(&NodeKey::from_packed(id.get()))?;
        (n.flags[1] != 0).then_some(n.address)
    }

    pub fn was_read(&self, key: NodeKey, address: usize) -> bool {
        self.nodes
            .get(&key)
            .is_some_and(|n| n.address == address && n.flags[2] != 0)
    }

    /// Enumeration is for reporting, not admission of a clean subtree.
    pub fn elements(&self) -> impl Iterator<Item = (NodeKey, usize)> + '_ {
        self.nodes
            .iter()
            .filter(|(_, n)| n.flags[0] != 0)
            .map(|(k, n)| (*k, n.address))
    }

    pub fn texts(&self) -> impl Iterator<Item = (TextId, usize)> + '_ {
        self.nodes
            .iter()
            .filter(|(_, n)| n.flags[1] != 0)
            .filter_map(|(k, n)| Some((TextId::new(k.packed())?, n.address)))
    }

    pub fn reads(&self) -> impl Iterator<Item = usize> + '_ {
        self.nodes
            .values()
            .filter(|n| n.flags[2] != 0)
            .map(|n| n.address)
    }
}
