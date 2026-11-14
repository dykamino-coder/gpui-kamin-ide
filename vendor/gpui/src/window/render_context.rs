//! Window-owned typed traversal scopes and snapshots for deferred drawing.
// KaminIDE patch: pass host paint/prepaint context through GPUI without thread-local state.

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

type Values = HashMap<TypeId, Rc<dyn Any>>;

#[derive(Clone, Default)]
pub(super) struct Snapshot(Values);

impl Snapshot {
    pub(super) fn get<T: Any>(&self) -> Option<Rc<T>> {
        self.0.get(&TypeId::of::<T>()).cloned()?.downcast().ok()
    }

    pub(super) fn set<T: Any>(&mut self, value: Option<Rc<T>>) {
        self.replace(value);
    }

    pub(super) fn replace<T: Any>(&mut self, value: Option<Rc<T>>) -> Option<Rc<T>> {
        let id = TypeId::of::<T>();
        let previous = match value {
            Some(value) => self.0.insert(id, value),
            None => self.0.remove(&id),
        };
        previous?.downcast().ok()
    }

    /// KaminIDE patch: copy `self` into `target`, reusing its table.
    pub(super) fn clone_into(&self, target: &mut Snapshot) {
        target.0.clone_from(&self.0);
    }

    /// KaminIDE patch: forget every value, keeping the table.
    pub(super) fn clear(&mut self) {
        self.0.clear();
    }
}

impl std::fmt::Debug for Snapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_set().entries(self.0.keys()).finish()
    }
}

impl PartialEq for Snapshot {
    fn eq(&self, other: &Self) -> bool {
        self.0.len() == other.0.len()
            && self.0.iter().all(|(id, value)| {
                other.0.get(id).is_some_and(|other| Rc::ptr_eq(value, other))
            })
    }
}

#[derive(Clone, Default)]
pub(super) struct RenderContexts(Rc<RefCell<Values>>);

impl RenderContexts {
    /// KaminIDE patch: a copy sized to the values, not to the live map. The live map keeps the
    /// capacity of its deepest scope after the scopes end (an empty map of eight buckets);
    /// a clone would copy those buckets into every paint snapshot, an empty one included.
    pub(super) fn snapshot(&self) -> Snapshot {
        Snapshot(self.0.borrow().iter().map(|(id, value)| (*id, value.clone())).collect())
    }

    pub(super) fn enter_snapshot(&self, snapshot: Snapshot) -> SnapshotScope {
        let previous = std::mem::replace(&mut *self.0.borrow_mut(), snapshot.0);
        SnapshotScope {
            values: self.0.clone(),
            previous,
            finished: false,
        }
    }

    /// KaminIDE patch: the current values are those of `snapshot` (same `Rc`s).
    pub(super) fn matches(&self, snapshot: &Snapshot) -> bool {
        let values = self.0.borrow();
        values.len() == snapshot.0.len()
            && values.iter().all(|(id, value)| {
                snapshot.0.get(id).is_some_and(|other| Rc::ptr_eq(value, other))
            })
    }

    pub(super) fn get<T: Any>(&self) -> Option<Rc<T>> {
        self.0
            .borrow()
            .get(&TypeId::of::<T>())
            .cloned()?
            .downcast()
            .ok()
    }

    pub(super) fn enter<T: Any>(&self, value: Option<Rc<T>>) -> Scope {
        let id = TypeId::of::<T>();
        let previous = match value {
            Some(value) => self.0.borrow_mut().insert(id, value),
            None => self.0.borrow_mut().remove(&id),
        };
        Scope {
            values: self.0.clone(),
            id,
            previous,
        }
    }
}

pub(super) struct SnapshotScope {
    values: Rc<RefCell<Values>>,
    previous: Values,
    finished: bool,
}

impl SnapshotScope {
    /// KaminIDE patch: restore the previous values and give the entered snapshot back,
    /// so that its table can be reused.
    pub(super) fn finish(mut self) -> Snapshot {
        let previous = std::mem::take(&mut self.previous);
        self.finished = true;
        Snapshot(std::mem::replace(&mut *self.values.borrow_mut(), previous))
    }
}

impl Drop for SnapshotScope {
    fn drop(&mut self) {
        if !self.finished {
            *self.values.borrow_mut() = std::mem::take(&mut self.previous);
        }
    }
}

pub(super) struct Scope {
    values: Rc<RefCell<Values>>,
    id: TypeId,
    previous: Option<Rc<dyn Any>>,
}

impl Drop for Scope {
    fn drop(&mut self) {
        let mut values = self.values.borrow_mut();
        match self.previous.take() {
            Some(previous) => {
                values.insert(self.id, previous);
            }
            None => {
                values.remove(&self.id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_scope_restores_the_same_outer_value() {
        let contexts = RenderContexts::default();
        let outer = Rc::new(17u32);
        let _outer = contexts.enter(Some(outer.clone()));
        {
            let _inner = contexts.enter(Some(Rc::new(23u32)));
            assert_eq!(*contexts.get::<u32>().unwrap(), 23);
        }
        assert!(Rc::ptr_eq(&contexts.get::<u32>().unwrap(), &outer));
    }

    #[test]
    fn unrelated_context_types_coexist() {
        let contexts = RenderContexts::default();
        let _number = contexts.enter(Some(Rc::new(17u32)));
        {
            let _text = contexts.enter(Some(Rc::new(String::from("fragment"))));
            assert_eq!(*contexts.get::<u32>().unwrap(), 17);
            assert_eq!(&*contexts.get::<String>().unwrap(), "fragment");
        }
        assert!(contexts.get::<String>().is_none());
        assert_eq!(*contexts.get::<u32>().unwrap(), 17);
    }

    #[test]
    fn empty_scope_temporarily_hides_the_parent() {
        let contexts = RenderContexts::default();
        let _outer = contexts.enter(Some(Rc::new(17u32)));
        {
            let _hidden = contexts.enter::<u32>(None);
            assert!(contexts.get::<u32>().is_none());
        }
        assert_eq!(*contexts.get::<u32>().unwrap(), 17);
    }

    #[test]
    fn windows_have_independent_contexts() {
        let first = RenderContexts::default();
        let second = RenderContexts::default();
        let _scope = first.enter(Some(Rc::new(17u32)));
        assert!(second.get::<u32>().is_none());
        let _scope = second.enter(Some(Rc::new(23u32)));
        assert_eq!(*first.get::<u32>().unwrap(), 17);
        assert_eq!(*second.get::<u32>().unwrap(), 23);
    }

    #[test]
    fn unwinding_restores_the_parent() {
        let contexts = RenderContexts::default();
        let _outer = contexts.enter(Some(Rc::new(17u32)));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _inner = contexts.enter(Some(Rc::new(23u32)));
            panic!("host traversal failed");
        }));
        assert!(result.is_err());
        assert_eq!(*contexts.get::<u32>().unwrap(), 17);
    }

    #[test]
    fn finished_scope_releases_values_and_keys() {
        let contexts = RenderContexts::default();
        let value = Rc::new(17u32);
        let weak = Rc::downgrade(&value);
        {
            let _scope = contexts.enter(Some(value));
            assert!(weak.upgrade().is_some());
        }
        assert!(weak.upgrade().is_none());
        assert!(contexts.0.borrow().is_empty());
    }

    #[test]
    fn captured_context_survives_its_scope_and_restores_the_caller() {
        let contexts = RenderContexts::default();
        let saved = {
            let _scope = contexts.enter(Some(Rc::new(17u32)));
            contexts.snapshot()
        };
        let _scope = contexts.enter(Some(Rc::new(23u32)));
        let _other = contexts.enter(Some(Rc::new(String::from("caller"))));
        {
            let _replay = contexts.enter_snapshot(saved);
            assert_eq!(*contexts.get::<u32>().unwrap(), 17);
            assert!(contexts.get::<String>().is_none());
            let _nested = contexts.enter(Some(Rc::new(29u32)));
            assert_eq!(*contexts.get::<u32>().unwrap(), 29);
        }
        assert_eq!(*contexts.get::<u32>().unwrap(), 23);
        assert_eq!(&*contexts.get::<String>().unwrap(), "caller");
    }

    #[test]
    fn a_snapshot_takes_the_room_of_its_values_not_of_the_live_map() {
        let contexts = RenderContexts::default();
        {
            let _a = contexts.enter(Some(Rc::new(1u8)));
            let _b = contexts.enter(Some(Rc::new(2u16)));
            let _c = contexts.enter(Some(Rc::new(3u32)));
            let _d = contexts.enter(Some(Rc::new(4u64)));
        }
        assert!(contexts.0.borrow().is_empty() && contexts.0.borrow().capacity() >= 4);
        assert_eq!(contexts.snapshot().0.capacity(), 0, "an empty snapshot allocates nothing");
        let _one = contexts.enter(Some(Rc::new(5u32)));
        assert!(contexts.snapshot().0.capacity() < 4);
    }

    #[test]
    fn unwinding_a_replay_restores_the_callers_context() {
        let contexts = RenderContexts::default();
        let _scope = contexts.enter(Some(Rc::new(17u32)));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _replay = contexts.enter_snapshot(Snapshot::default());
            assert!(contexts.get::<u32>().is_none());
            panic!("deferred traversal failed");
        }));
        assert!(result.is_err());
        assert_eq!(*contexts.get::<u32>().unwrap(), 17);
    }
}
