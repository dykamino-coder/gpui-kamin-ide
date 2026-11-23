//! The full walk and a dirty owner record the same direct segment payload.
use super::*;
use crate::budget::admission::{Admission, Error};
use markup5ever_rcdom::Handle;

#[derive(Default)]
pub(super) struct Draft {
    pub(super) owner: NodeKey,
    pub(super) direct: HashMap<NodeKey, Binding>,
}

/// Only direct observations from a walk; not an update of a committed store.
pub struct BindingRecording {
    pub(super) changed: HashMap<NodeKey, Rc<BindingSegment>>,
}

#[derive(Default)]
pub struct BindingRecorder {
    pub(super) stack: Vec<Draft>,
    pub(super) root: Draft,
    pub(super) changed: HashMap<NodeKey, Rc<BindingSegment>>,
    pub(super) text_addresses: HashMap<TextId, usize>,
}

impl BindingRecorder {
    pub fn begin_owner(&mut self, owner: NodeKey) {
        // Re-entering an owner invalidates its previous recording capability.
        // An out-of-order finish must not silently return the old segment.
        self.changed.remove(&owner);
        self.stack.push(Draft {
            owner,
            ..Default::default()
        });
    }

    pub fn end_owner(&mut self, admission: &Admission) -> Result<Rc<BindingSegment>, Error> {
        let draft = self.stack.pop().expect("paired source owner");
        self.finish(draft, admission)
    }

    /// Finish the active direct owner, or fetch its already finished capability.
    /// Descendant observations are recorded in their own drafts, never captured here.
    pub fn finish_owner(
        &mut self,
        owner: NodeKey,
        admission: &Admission,
    ) -> Result<Rc<BindingSegment>, Error> {
        if self.stack.last().is_some_and(|draft| draft.owner == owner) {
            self.end_owner(admission)
        } else {
            Ok(self
                .changed
                .get(&owner)
                .expect("finished source owner")
                .clone())
        }
    }

    pub fn segment(&self, owner: NodeKey) -> Option<Rc<BindingSegment>> {
        self.changed.get(&owner).cloned()
    }

    fn finish(&mut self, draft: Draft, admission: &Admission) -> Result<Rc<BindingSegment>, Error> {
        let bytes = draft
            .direct
            .len()
            .checked_mul(std::mem::size_of::<(NodeKey, Binding)>())
            .and_then(|n| n.checked_add(std::mem::size_of::<BindingSegment>()))
            .ok_or(Error::CapacityOverflow)?;
        let lease = admission.storage(bytes)?;
        self.changed.try_reserve(1).map_err(|_| Error::Allocation)?;
        let mut direct = Vec::new();
        direct
            .try_reserve_exact(draft.direct.len())
            .map_err(|_| Error::Allocation)?;
        direct.extend(draft.direct);
        let segment = Rc::new(BindingSegment {
            owner: draft.owner,
            direct: direct.into_boxed_slice(),
            _lease: lease,
        });
        self.changed.insert(segment.owner, segment.clone());
        Ok(segment)
    }

    fn record(&mut self, key: NodeKey, address: usize, flag: u8) {
        let draft = self.stack.last_mut().unwrap_or(&mut self.root);
        let entry = draft
            .direct
            .entry(key)
            .or_insert(Binding { address, flags: 0 });
        assert_eq!(entry.address, address, "a source key cannot name two nodes");
        entry.flags |= flag;
    }

    pub fn element(&mut self, handle: &Handle) {
        self.record(handle.key(), Rc::as_ptr(handle) as usize, ELEMENT);
    }

    pub fn text(&mut self, handle: &Handle) -> Option<TextId> {
        let id = TextId::new(handle.key().packed())?;
        let address = Rc::as_ptr(handle) as usize;
        self.record(handle.key(), address, TEXT);
        self.text_addresses.insert(id, address);
        Some(id)
    }

    pub fn read(&mut self, handle: &Handle) {
        self.record(handle.key(), Rc::as_ptr(handle) as usize, READ);
    }

    pub fn read_text(&mut self, id: TextId, committed: &BindingStore) {
        if let Some(address) = self
            .text_addresses
            .get(&id)
            .copied()
            .or_else(|| committed.text(id))
        {
            self.record(NodeKey::from_packed(id.get()), address, READ);
        }
    }

    pub fn into_recording(mut self, admission: &Admission) -> Result<BindingRecording, Error> {
        assert!(self.stack.is_empty(), "unfinished source owner");
        let root = std::mem::take(&mut self.root);
        if !root.direct.is_empty() {
            self.finish(root, admission)?;
        }
        Ok(BindingRecording {
            changed: self.changed,
        })
    }
}
