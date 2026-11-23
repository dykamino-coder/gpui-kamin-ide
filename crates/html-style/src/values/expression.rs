//! Immutable CSS expressions live with their actual readers, never a document arena.

use std::ops::Deref;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Stable identity of one allocation; equality of CSS values remains semantic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ValueId(u64);

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
struct Expression<T> {
    id: ValueId,
    value: T,
}

/// Owning handle shared by computed groups, staged changes and retained frames.
pub struct ValueRef<T>(Rc<Expression<T>>);

impl<T> ValueRef<T> {
    pub fn new(value: T) -> Self {
        let id = NEXT_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .expect("expression identity exhausted");
        Self(Rc::new(Expression {
            id: ValueId(id),
            value,
        }))
    }

    pub fn id(&self) -> ValueId {
        self.0.id
    }

    pub fn read(&self) -> &T {
        &self.0.value
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for ValueRef<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.read().fmt(f)
    }
}

impl<T> Clone for ValueRef<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Deref for ValueRef<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.read()
    }
}

impl<T: PartialEq> PartialEq for ValueRef<T> {
    fn eq(&self, other: &Self) -> bool {
        self.read() == other.read()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_shared_but_equality_is_semantic() {
        let first = ValueRef::new(12);
        let shared = first.clone();
        let equal = ValueRef::new(12);
        assert_eq!(first.id(), shared.id());
        assert_ne!(first.id(), equal.id());
        assert_eq!(first, equal);
    }

    #[test]
    fn expressions_do_not_retain_their_document() {
        use crate::style::css::Media;
        use crate::style::values::value::{Len, anchor_get, calc_get};
        use crate::values::DocumentValues;
        let document = DocumentValues::detached(Media::default());
        let weak_document = Rc::downgrade(&document);
        let calc = Len::parse("calc(3ch + 7px)", document.values()).unwrap();
        let anchor = Len::parse(
            "anchor(--target bottom, calc(3ch + 3px))",
            document.values(),
        )
        .unwrap();
        drop(document);
        assert!(weak_document.upgrade().is_none());
        let next = DocumentValues::detached(Media::default());
        let Len::Calc(calc) = calc else {
            panic!("calc expression")
        };
        let sum = calc_get(&calc, next.values());
        assert_eq!((sum.ch, sum.px), (3.0, 7.0));
        let Len::Anchor(anchor) = anchor else {
            panic!("anchor expression")
        };
        let description = anchor_get(&anchor, next.values()).unwrap();
        assert_eq!(description.name.as_deref(), Some("--target"));
        let Some(Len::Calc(fallback)) = &description.fallback else {
            panic!("fallback")
        };
        let sum = calc_get(fallback, next.values());
        assert_eq!((sum.ch, sum.px), (3.0, 3.0));
    }

    #[test]
    fn ten_thousand_mutations_release_replaced_and_aborted_expressions() {
        use crate::style::css::Media;
        use crate::style::values::value::Len;
        use crate::values::{DocumentValues, ValueCx};
        use std::any::Any;
        use std::rc::Weak;
        fn observe<T: 'static>(value: &ValueRef<T>) -> Weak<dyn Any> {
            let erased: Rc<dyn Any> = value.0.clone();
            Rc::downgrade(&erased)
        }
        fn readers(values: &[Len]) -> Vec<Weak<dyn Any>> {
            let mut result = Vec::new();
            for value in values {
                match value {
                    Len::Calc(value) => result.push(observe(value)),
                    Len::Anchor(value) => {
                        result.push(observe(value));
                        if let Some(fallback) = &value.fallback {
                            result.extend(readers(std::slice::from_ref(fallback)));
                        }
                    }
                    _ => panic!("expected an owning expression"),
                }
            }
            result
        }
        fn parse(index: usize, vcx: ValueCx<'_>) -> [Len; 2] {
            [
                Len::parse(&format!("calc(3ch + {index}px)"), vcx).unwrap(),
                Len::parse(
                    &format!("anchor(--target bottom, calc(3ch + {index}px))"),
                    vcx,
                )
                .unwrap(),
            ]
        }
        let document = DocumentValues::detached(Media::default());
        let held_frame = parse(1, document.values());
        let held_readers = readers(&held_frame);
        let mut current = held_frame.clone();
        for index in 2..=10_001 {
            let stage = parse(index, document.values());
            let staged_readers = readers(&stage);
            drop(stage);
            assert!(staged_readers.iter().all(|value| value.upgrade().is_none()));
            let previous = readers(&current);
            current = parse(index, document.values());
            if index > 2 {
                assert!(previous.iter().all(|value| value.upgrade().is_none()));
            }
            assert!(held_readers.iter().all(|value| value.upgrade().is_some()));
        }
        drop(document);
        assert!(held_readers.iter().all(|value| value.upgrade().is_some()));
        drop(held_frame);
        assert!(held_readers.iter().all(|value| value.upgrade().is_none()));
        let last = readers(&current);
        drop(current);
        assert!(last.iter().all(|value| value.upgrade().is_none()));
    }
}
