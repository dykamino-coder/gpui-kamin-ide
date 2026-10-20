//! Native font installation identity, scoped to one platform loader/backend.

use crate::text::fonts::Loader;
use std::{cell::RefCell, collections::HashMap};

pub(super) fn deduplicated(loader: Loader) -> Loader {
    // Reinstalling an identical native face changes the platform collection.
    // Full byte equality shares the resource without sharing document URL state.
    let installed = RefCell::new(HashMap::<Vec<u8>, String>::new());
    Box::new(move |bytes| {
        if let Some(family) = installed.borrow().get(&bytes) {
            return Some(family.clone());
        }
        let family = loader(bytes.clone())?;
        installed.borrow_mut().insert(bytes, family.clone());
        Some(family)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};

    #[test]
    fn native_identity_uses_complete_bytes_and_backend_lifetime() {
        let calls = Rc::new(Cell::new(0));
        let make = || {
            let calls = calls.clone();
            deduplicated(Box::new(move |bytes| {
                calls.set(calls.get() + 1);
                Some(format!("native-{}", bytes[0]))
            }))
        };
        let first = make();
        assert_eq!(first(vec![1, 2]), Some("native-1".into()));
        assert_eq!(first(vec![1, 2]), Some("native-1".into()));
        assert_eq!(calls.get(), 1);
        assert_eq!(first(vec![1, 3]), Some("native-1".into()));
        assert_eq!(calls.get(), 2);
        let second = make();
        assert_eq!(second(vec![1, 2]), Some("native-1".into()));
        assert_eq!(calls.get(), 3);
    }

    #[test]
    fn failed_installations_are_retried() {
        let calls = Rc::new(Cell::new(0));
        let count = calls.clone();
        let load = deduplicated(Box::new(move |_| {
            count.set(count.get() + 1);
            (count.get() > 1).then(|| "native".into())
        }));
        assert_eq!(load(vec![1]), None);
        assert_eq!(load(vec![1]), Some("native".into()));
        assert_eq!(load(vec![1]), Some("native".into()));
        assert_eq!(calls.get(), 2);
    }
}
