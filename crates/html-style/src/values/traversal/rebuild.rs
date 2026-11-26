//! A walk of one subtree of the built document (`dom::walk::subtree`): the
//! elements it builds are recorded apart, so that the document's list holds
//! each address once, and a source text keeps the number the build gave it
//! (`TextId` numbers texts in the order of the document walk, which a walk of
//! one subtree does not repeat). A text the document walk did not reach
//! (inside an element that had no box) takes the next free number.

use super::TraversalState;
use markup5ever_rcdom::Handle;
use std::collections::HashMap;
use std::rc::Rc;

/// The records of the subtree walks of one edit.
pub(super) struct Rebuild {
    /// The number of each source text by address.
    texts: HashMap<usize, u32>,
    /// The elements the current walk built.
    elements: Vec<usize>,
}

impl TraversalState {
    /// Start the subtree walks of one edit.
    pub fn begin_rebuild(&self) {
        let texts = self.texts.borrow();
        let texts = (0..texts.len()).map(|i| (texts[i], i as u32 + 1)).collect();
        self.rebuild.replace(Some(Rebuild {
            texts,
            elements: Vec::new(),
        }));
    }

    /// The elements the walk since the last call built.
    pub fn take_rebuilt(&self) -> Vec<usize> {
        let mut rebuild = self.rebuild.borrow_mut();
        rebuild
            .as_mut()
            .map(|r| std::mem::take(&mut r.elements))
            .unwrap_or_default()
    }

    /// End the subtree walks of one edit; `built` — the elements the kept
    /// walks built, `dropped` — those of the boxes they replace.
    pub fn end_rebuild(&self, dropped: &[usize], built: &[usize]) {
        self.rebuild.replace(None);
        let dropped: std::collections::HashSet<usize> = dropped.iter().copied().collect();
        let mut elements = self.elements.borrow_mut();
        elements.retain(|a| !dropped.contains(a));
        elements.extend_from_slice(built);
    }

    /// Leave the subtree walks of one edit with the document's records as
    /// they were (the edit rebuilds the document).
    pub fn abandon_rebuild(&self) {
        self.rebuild.replace(None);
    }

    pub(super) fn rebuilt_text(&self, handle: &Handle) -> Option<crate::dom::TextId> {
        let rebuild = self.rebuild.borrow();
        let id = *rebuild
            .as_ref()?
            .texts
            .get(&(Rc::as_ptr(handle) as usize))?;
        crate::dom::TextId::new(id)
    }

    pub(super) fn note_text(&self, handle: &Handle, id: u32) {
        if let Some(rebuild) = self.rebuild.borrow_mut().as_mut() {
            rebuild.texts.insert(Rc::as_ptr(handle) as usize, id);
        }
    }

    /// Record the element in the current subtree walk; `false` outside one.
    pub(super) fn rebuilt_element(&self, handle: &Handle) -> bool {
        match self.rebuild.borrow_mut().as_mut() {
            Some(rebuild) => {
                rebuild.elements.push(Rc::as_ptr(handle) as usize);
                true
            }
            None => false,
        }
    }
}
