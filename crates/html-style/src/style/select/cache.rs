//! Кэши одного исходного DOM: ревизия и область якоря входят во время жизни.

use super::{Ancestor, Sibs};
use crate::budget::admission::{Admission, Error};
use crate::dom::tree_index::NodeKey;
use std::{cell::RefCell, collections::HashMap, rc::Rc};

type NthKey = (NodeKey, u8, String);
struct NthEntry {
    ranks: Rc<Vec<usize>>,
    _lease: Option<crate::budget::LiveReservation>,
}

pub struct MatchCaches {
    doc: Rc<crate::values::DocumentValues>,
    admission: Admission,
    anchor: RefCell<Option<NodeKey>>,
    nth: RefCell<HashMap<NthKey, NthEntry>>,
}

pub struct AnchorScope<'a> {
    caches: &'a MatchCaches,
    previous: Option<NodeKey>,
}
impl Drop for AnchorScope<'_> {
    fn drop(&mut self) {
        *self.caches.anchor.borrow_mut() = self.previous.take();
    }
}

impl MatchCaches {
    pub fn new(doc: Rc<crate::values::DocumentValues>) -> Self {
        let admission = doc
            .budget
            .borrow()
            .clone()
            .map(Admission::new)
            .unwrap_or_default();
        Self {
            doc,
            admission,
            anchor: RefCell::new(None),
            nth: RefCell::new(HashMap::new()),
        }
    }
    pub(super) fn accept<T>(&self, result: Result<T, Error>) -> Option<T> {
        result
            .map_err(|error| self.doc.revision_budget().reject(error.into()))
            .ok()
    }
    pub fn work(&self, count: usize) -> bool {
        self.accept(self.admission.work(count)).is_some()
    }
    pub fn anchor(&self, key: NodeKey) -> AnchorScope<'_> {
        AnchorScope {
            caches: self,
            previous: self.anchor.replace(Some(key)),
        }
    }
    pub fn is_anchor(&self, key: &NodeKey) -> bool {
        self.anchor.borrow().as_ref() == Some(key)
    }
    pub fn has_anchor(&self) -> bool {
        self.anchor.borrow().is_some()
    }

    /// Вектор рангов строится вне borrow карты: S может содержать другую nth-форму.
    pub fn nth_ranks(
        &self,
        namespace: u8,
        raw: &str,
        sibs: Sibs,
        hit: impl Fn(&Ancestor, Sibs) -> bool,
    ) -> Option<Rc<Vec<usize>>> {
        let key = (sibs.all.first()?.key.clone(), namespace, raw.to_string());
        if let Some(entry) = self.nth.borrow().get(&key) {
            return Some(entry.ranks.clone());
        }
        if !self.work(sibs.all.len()) {
            return None;
        }
        let bytes = self.accept(
            sibs.all
                .len()
                .checked_add(1)
                .and_then(|n| n.checked_mul(std::mem::size_of::<usize>()))
                .and_then(|n| n.checked_add(raw.len()))
                .and_then(|n| n.checked_add(128))
                .ok_or(Error::CapacityOverflow),
        )?;
        let lease = self.accept(self.admission.storage(bytes))?;
        let mut ranks = Vec::new();
        self.accept(
            ranks
                .try_reserve_exact(sibs.all.len() + 1)
                .map_err(|_| Error::Allocation),
        )?;
        let mut rank = 0;
        for (i, node) in sibs.all.iter().enumerate() {
            let matched = hit(node, sibs.at(i));
            rank += usize::from(matched);
            ranks.push(if matched { rank } else { 0 });
        }
        ranks.push(rank);
        let ranks = Rc::new(ranks);
        let mut cache = self.nth.borrow_mut();
        self.accept(cache.try_reserve(1).map_err(|_| Error::Allocation))?;
        cache.insert(
            key,
            NthEntry {
                ranks: ranks.clone(),
                _lease: lease,
            },
        );
        Some(ranks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn nth_cache_counts_once_per_sibling_owner_and_never_crosses_a_tree() {
        let dom = crate::dom::xml::xml_dom("<r><a><x/><y/><x/></a><b><y/><x/></b></r>").unwrap();
        let doc = crate::values::DocumentValues::detached(Default::default());
        let tree = crate::dom::tree_index::TreeIndex::new(doc.clone(), true);
        let root = dom.document.children.borrow()[0].clone();
        let parents = root.children.borrow();
        let calls = Cell::new(0);
        for (parent, expected) in parents.iter().zip([vec![1, 0, 2, 2], vec![0, 1, 1]]) {
            let (_, all) = super::super::census_of(&parent.children.borrow(), &tree);
            let sibs = Sibs {
                all: &all,
                pos: 0,
                is_elem: true,
                rc: None,
            };
            for _ in 0..all.len() {
                let ranks = tree
                    .matching
                    .nth_ranks(0, "of x", sibs, |node, _| {
                        calls.set(calls.get() + 1);
                        node.tag == "x"
                    })
                    .unwrap();
                assert_eq!(*ranks, expected);
            }
        }
        assert_eq!(calls.get(), 5);
        let fresh = crate::dom::tree_index::TreeIndex::new(doc, true);
        assert!(fresh.matching.nth.borrow().is_empty());
    }

    #[test]
    fn nested_anchor_scopes_restore_the_previous_identity() {
        let dom = crate::dom::xml::xml_dom("<r><x/></r>").unwrap();
        let r = dom.document.children.borrow()[0].clone();
        let x = r.children.borrow()[0].clone();
        let caches = MatchCaches::new(crate::values::DocumentValues::detached(Default::default()));
        let outer = caches.anchor(NodeKey::of(&r));
        {
            let _inner = caches.anchor(NodeKey::of(&x));
            assert!(caches.is_anchor(&NodeKey::of(&x)));
        }
        assert!(caches.is_anchor(&NodeKey::of(&r)));
        drop(outer);
        assert!(!caches.has_anchor());
    }

    #[test]
    fn nth_cache_separates_namespaces_and_new_revision_after_rejection() {
        use crate::budget::{BudgetKind, DocumentBudget, DocumentLimits, ResourceGovernor};
        let dom = crate::dom::xml::xml_dom("<r><x/></r>").unwrap();
        let doc = crate::values::DocumentValues::detached(Default::default());
        let owner =
            DocumentBudget::new(DocumentLimits::default(), ResourceGovernor::default()).unwrap();
        let revision = owner.revision();
        doc.admit_revision(revision.clone(), None);
        let tree = crate::dom::tree_index::TreeIndex::new(doc.clone(), true);
        let root = dom.document.children.borrow()[0].clone();
        let (_, all) = super::super::census_of(&root.children.borrow(), &tree);
        let sibs = Sibs {
            all: &all,
            pos: 0,
            is_elem: true,
            rc: None,
        };
        assert_eq!(
            *tree
                .matching
                .nth_ranks(0, "of x", sibs, |_, _| true)
                .unwrap(),
            [1, 1]
        );
        assert_eq!(
            *tree
                .matching
                .nth_ranks(1, "of x", sibs, |_, _| false)
                .unwrap(),
            [0, 0]
        );
        let remaining = owner.limits().limit(BudgetKind::TraversalVisits)
            - revision.used(BudgetKind::TraversalVisits);
        revision
            .consume(BudgetKind::TraversalVisits, remaining)
            .unwrap();
        assert!(
            tree.matching
                .nth_ranks(0, "of y", sibs, |_, _| true)
                .is_none()
        );
        assert!(revision.failure().is_some());
        assert_eq!(tree.matching.nth.borrow().len(), 2);
        let fresh = owner.revision();
        doc.admit_revision(fresh.clone(), None);
        let caches = MatchCaches::new(doc);
        assert_eq!(
            *caches.nth_ranks(0, "of x", sibs, |_, _| false).unwrap(),
            [0, 0]
        );
        assert!(fresh.failure().is_none());
    }
}
