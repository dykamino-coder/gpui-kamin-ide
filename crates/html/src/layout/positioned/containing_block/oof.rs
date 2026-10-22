//! Носители внепоточных детей слоя CB, без новой коробки в дереве GPUI.

use super::SpotCell;
use gpui::AnyElement;
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

mod deferred;
use deferred::DeferredOof;

pub(super) struct OofKid {
    spot: SpotCell,
    element: AnyElement,
    deferred: bool,
}

type Storage = RefCell<Vec<OofKid>>;

#[derive(Clone)]
pub(super) struct OofHost {
    _owner: Option<Rc<Storage>>,
    storage: Weak<Storage>,
}

impl Default for OofHost {
    fn default() -> Self {
        let owner = Rc::new(RefCell::new(Vec::new()));
        Self {
            storage: Rc::downgrade(&owner),
            _owner: Some(owner),
        }
    }
}

impl OofHost {
    pub(super) fn borrowed(&self) -> Self {
        Self {
            _owner: None,
            storage: self.storage.clone(),
        }
    }

    pub(super) fn discard() -> Self {
        Self {
            _owner: None,
            storage: Weak::new(),
        }
    }

    fn push_kid(&mut self, kid: OofKid) {
        if let Some(storage) = self.storage.upgrade() {
            storage.borrow_mut().push(kid);
        }
    }

    pub(super) fn clear(&self) {
        if let Some(storage) = self.storage.upgrade() {
            storage.borrow_mut().clear();
        }
    }
    pub(super) fn defer(&mut self) -> Self {
        let pending = Self::default();
        self.push_kid(OofKid {
            spot: Default::default(),
            element: crate::hooks::IntoHooked::hooked(DeferredOof::new(pending.clone())),
            deferred: true,
        });
        pending
    }
    pub(super) fn push(&mut self, spot: SpotCell, element: AnyElement) {
        self.push_kid(OofKid {
            spot,
            element,
            deferred: false,
        });
    }

    pub(super) fn is_empty(&self) -> bool {
        self.storage.upgrade().is_none_or(|s| s.borrow().is_empty())
    }

    pub(super) fn place(self, place: fn(SpotCell, AnyElement) -> AnyElement) -> Vec<AnyElement> {
        self.storage
            .upgrade()
            .map(|s| std::mem::take(&mut *s.borrow_mut()))
            .unwrap_or_default()
            .into_iter()
            .map(|kid| {
                if kid.deferred {
                    kid.element
                } else {
                    place(kid.spot, kid.element)
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{IntoElement, div};

    #[test]
    fn borrowed_destination_does_not_keep_its_candidates_alive() {
        let owner = OofHost::default();
        let mut borrower = owner.borrowed();
        borrower.push(Default::default(), div().into_any_element());
        assert!(!owner.is_empty());
        drop(owner);
        assert!(borrower.is_empty());
        borrower.push(Default::default(), div().into_any_element());
        assert!(borrower.is_empty());
    }
}
