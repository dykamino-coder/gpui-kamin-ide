//! A document's decimal identity namespace, independently retained by style passes.

use super::DocumentValues;
use std::cell::RefCell;
use std::rc::Rc;

/// Retains only the decimal counter, without document, DOM, or frame ownership.
#[derive(Clone, Default)]
pub struct SyntheticIds(Rc<RefCell<Vec<u8>>>);

impl SyntheticIds {
    pub fn same_allocator(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    pub fn next(&self) -> String {
        // Least-significant digit first; the empty state denotes zero.
        let mut digits = self.0.borrow_mut();
        if digits.is_empty() {
            digits.push(0);
        }
        let id = digits.iter().rev().map(|d| char::from(b'0' + d)).collect();
        for digit in digits.iter_mut() {
            if *digit < 9 {
                *digit += 1;
                return id;
            }
            *digit = 0;
        }
        digits.push(1);
        id
    }
}

impl DocumentValues {
    pub fn synthetic_ids(&self) -> SyntheticIds {
        self.synthetic_id.clone()
    }

    pub fn next_synthetic_id(&self) -> String {
        self.synthetic_id.next()
    }
}

#[cfg(any(test, feature = "engine-test-support"))]
pub mod inventory_cases {
    use crate::values::DocumentValues;

    pub fn carry_extends_the_key_without_reusing_zero() {
        let doc = DocumentValues::detached(Default::default());
        assert_eq!(doc.next_synthetic_id(), "0");
        *doc.synthetic_id.0.borrow_mut() = vec![9; 24];
        assert_eq!(doc.next_synthetic_id(), "9".repeat(24));
        assert_eq!(doc.next_synthetic_id(), format!("1{}", "0".repeat(24)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interleaved_handles_share_the_document_namespace() {
        let doc = DocumentValues::detached(Default::default());
        let ids = doc.synthetic_ids();
        let cloned = ids.clone();
        assert!(ids.same_allocator(&cloned));
        assert!(ids.same_allocator(&doc.synthetic_ids()));
        assert_eq!(doc.next_synthetic_id(), "0");
        assert_eq!(ids.next(), "1");
        assert_eq!(cloned.next(), "2");
        assert_eq!(doc.next_synthetic_id(), "3");
    }

    #[test]
    fn retained_handle_releases_document_and_final_counter() {
        let doc = DocumentValues::detached(Default::default());
        let document = Rc::downgrade(&doc);
        let ids = doc.synthetic_ids();
        let counter = Rc::downgrade(&ids.0);
        assert_eq!(doc.next_synthetic_id(), "0");
        drop(doc);
        assert!(document.upgrade().is_none());
        assert_eq!(ids.next(), "1");
        drop(ids);
        assert!(counter.upgrade().is_none());
    }

    #[test]
    fn independent_documents_have_distinct_allocators() {
        let first = DocumentValues::detached(Default::default()).synthetic_ids();
        let second = DocumentValues::detached(Default::default()).synthetic_ids();
        assert!(!first.same_allocator(&second));
        assert_eq!(first.next(), "0");
        assert_eq!(second.next(), "0");
    }
}
