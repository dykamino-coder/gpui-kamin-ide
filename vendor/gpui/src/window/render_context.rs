//! Window-owned typed traversal scopes and snapshots for deferred drawing.
// KaminIDE patch: pass host paint/prepaint context through GPUI without thread-local state.

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

type Values = HashMap<TypeId, Rc<dyn Any>>;

#[derive(Clone, Default)]
pub(super) struct Snapshot(Values);

#[derive(Default)]
pub(super) struct RenderContexts(Rc<RefCell<Values>>);

impl RenderContexts {
    pub(super) fn snapshot(&self) -> Snapshot {
        Snapshot(self.0.borrow().clone())
    }

    pub(super) fn enter_snapshot(&self, snapshot: Snapshot) -> SnapshotScope {
        let previous = std::mem::replace(&mut *self.0.borrow_mut(), snapshot.0);
        SnapshotScope {
            values: self.0.clone(),
            previous,
        }
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
}

impl Drop for SnapshotScope {
    fn drop(&mut self) {
        *self.values.borrow_mut() = std::mem::take(&mut self.previous);
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
