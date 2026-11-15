//! The addressable source elements of a build, by address (`SourceIndex`):
//! what an edit in place reads for each mutated or restyled element instead
//! of a scan of the build's address list. An addressed element is named by
//! its stable node key (`Handle::key`), as its boxes are.
//!
//! Blink reaches the layout object of a node through a pointer on the node
//! (`Node::GetLayoutObject`, `node.h:866`) and Stylo the data of an element
//! through its own slot (`TElement::borrow_data`, `dom.rs:724`). The source
//! DOM here carries no slot of the engine: the index is a sorted list of
//! (address, number) pairs, built on the first edit in place of a build and
//! searched in O(log N), 16 bytes per element.

use std::rc::Rc;

/// A number that marks an address the list holds twice: such an element is
/// not addressed by an edit.
const TWICE: u32 = 0;

/// The box numbers of the addresses of a build (`TraversalState::elements`).
pub(in crate::document) struct SourceIndex {
    pairs: Vec<(usize, u32)>,
}

impl SourceIndex {
    /// The index of `list`, where the number of the address at slot `i` is
    /// `i + 1`; `None` if the list has more slots than numbers.
    pub(in crate::document) fn of(list: &[usize]) -> Option<Self> {
        let mut pairs = Vec::with_capacity(list.len());
        for (slot, &address) in list.iter().enumerate() {
            pairs.push((address, u32::try_from(slot + 1).ok()?));
        }
        pairs.sort_unstable();
        let mut out: Vec<(usize, u32)> = Vec::with_capacity(pairs.len());
        for (address, id) in pairs {
            match out.last_mut() {
                Some(last) if last.0 == address => last.1 = TWICE,
                _ => out.push((address, id)),
            }
        }
        out.shrink_to_fit();
        Some(Self { pairs: out })
    }

    /// The node key of the source node `handle`: `None` if no box was built
    /// for it or its address has two numbers.
    pub(in crate::document) fn id(&self, handle: &markup5ever_rcdom::Handle) -> Option<u64> {
        let address = Rc::as_ptr(handle) as usize;
        let at = self.pairs.binary_search_by_key(&address, |p| p.0).ok()?;
        (self.pairs[at].1 != TWICE).then(|| handle.key().packed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_slots_plus_one_and_twice_held_addresses_are_unaddressed() {
        let index = SourceIndex::of(&[40, 8, 40, 16]).unwrap();
        let pairs: Vec<_> = index.pairs.iter().copied().collect();
        assert_eq!(pairs, vec![(8, 2), (16, 4), (40, TWICE)]);
    }

    #[test]
    fn the_index_agrees_with_the_scan_of_the_list() {
        let doc =
            crate::document::Document::builder("<div><p>a</p><p>b<span>c</span></p></div>", "")
                .build()
                .unwrap();
        let root = doc.source_dom();
        let list = doc.state.values.traversal.elements().clone();
        let index = SourceIndex::of(&list).unwrap();
        let mut pending = vec![root];
        let mut seen = 0;
        while let Some(node) = pending.pop() {
            let scan = super::super::adopt::element_ids(&list, std::iter::once(&node));
            assert_eq!(index.id(&node), scan[0]);
            seen += usize::from(scan[0].is_some());
            pending.extend(node.children.borrow().iter().cloned());
        }
        assert!(seen >= 4, "{seen}");
    }
}
