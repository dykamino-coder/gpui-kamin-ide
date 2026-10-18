//! Document-owned fallback measures for rotated atomic subtrees.
//!
//! Native vertical Paragraphs and definite orthogonal inline constraints already
//! measure in one pass. The rotated atomic adapter still claims its block size
//! before its flex parent resolves the final inline length. Prepaint lays out
//! the flat child at that length and records the resulting block size for the
//! next frame. The claim-cap and fit-limit paths also retain this fallback.
//!
//! A copied GPUI LayoutMeasurement can measure without borrowing the parent's
//! engine. Using it here requires replacing the fixed width claim with a
//! measured leaf and resolving auto-height against the containing block or ICB
//! (CSS Writing Modes 4 section 7.3), rather than its current parent stretch.
//! Moving the built AnyElement into a nested engine is invalid: descendants
//! retain LayoutIds from the original tree. This module only isolates the
//! existing fallback; it does not make that sizing contract one-pass.
//! Storage and occurrence numbering belong to one document, including iframes.

use gpui::Pixels;
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Default)]
pub(crate) struct VerticalMeasures {
    sizes: RefCell<HashMap<u64, Pixels>>,
    occurrences: RefCell<HashMap<u64, u64>>,
}

impl VerticalMeasures {
    pub(crate) fn begin_frame(&self) {
        self.occurrences.borrow_mut().clear();
    }

    pub(crate) fn next_key(&self, base: u64) -> u64 {
        let mut counts = self.occurrences.borrow_mut();
        let n = counts.entry(base).or_default();
        *n += 1;
        base ^ n.wrapping_mul(0x517C_C1B7_2722_0A95)
    }

    pub(crate) fn get(&self, key: u64) -> Option<Pixels> {
        self.sizes.borrow().get(&key).copied()
    }

    pub(crate) fn record(&self, key: u64, size: Pixels) {
        let mut sizes = self.sizes.borrow_mut();
        if sizes.len() >= 256 {
            sizes.clear();
        }
        sizes.insert(key, size);
    }

    pub(crate) fn clear(&self) {
        self.sizes.borrow_mut().clear();
        self.begin_frame();
    }
}

#[cfg(test)]
mod tests {
    use crate::document::state::{DocState, current};
    use gpui::px;

    #[test]
    fn measures_follow_document_scope_and_occurrences_survive_frames() {
        let outer = DocState::new(Default::default());
        let inner = DocState::new(Default::default());
        let _outer = outer.enter_frame();
        let a = current().vertical_measures.next_key(7);
        let b = current().vertical_measures.next_key(7);
        assert_ne!(a, b);
        current().vertical_measures.record(a, px(20.));
        current().vertical_measures.record(b, px(40.));
        {
            let _inner = inner.enter_frame();
            assert_eq!(current().vertical_measures.next_key(7), a);
            assert_eq!(current().vertical_measures.get(a), None);
            current().vertical_measures.record(a, px(80.));
        }
        current().vertical_measures.begin_frame();
        assert_eq!(current().vertical_measures.next_key(7), a);
        assert_eq!(current().vertical_measures.get(a), Some(px(20.)));
        assert_eq!(current().vertical_measures.next_key(7), b);
        assert_eq!(current().vertical_measures.get(b), Some(px(40.)));
        current().vertical_measures.clear();
        assert_eq!(current().vertical_measures.get(a), None);
        assert_eq!(inner.vertical_measures.get(a), Some(px(80.)));
    }
}
