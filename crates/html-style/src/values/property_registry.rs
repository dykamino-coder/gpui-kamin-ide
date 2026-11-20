//! Неизменяемые снимки @property документа (CSS Properties & Values §3).

use crate::style::css::Registered;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone, Default)]
pub struct PropertySnapshot {
    pub revision: u64,
    pub rules: HashMap<String, Registered>,
    /// Registered properties that do not inherit (inherits: false).
    pub non_inherited: usize,
    /// Inheriting registered properties with an initial value: a computed
    /// custom property map has a value for each of them.
    pub inherited_initial: Vec<String>,
}

#[derive(Default)]
pub struct PropertyRegistry {
    current: Rc<PropertySnapshot>,
}

impl PropertyRegistry {
    pub fn snapshot(&self) -> Rc<PropertySnapshot> {
        Rc::clone(&self.current)
    }

    pub fn register(&mut self, name: &str, registration: Registered) {
        if self.current.rules.contains_key(name) {
            return;
        }
        // Читатель удерживает прежнюю ревизию без заимствования RefCell.
        // Обычный обход стилей лишь увеличивает счётчик ссылок.
        let next = Rc::make_mut(&mut self.current);
        if !registration.inherits {
            next.non_inherited += 1;
        } else if registration.initial.is_some() {
            next.inherited_initial.push(name.to_string());
        }
        next.rules.insert(name.to_string(), registration);
        next.revision += 1;
    }
}

#[cfg(any(test, feature = "engine-test-support"))]
pub mod inventory_cases {
    use super::*;

    fn registration() -> Registered {
        Registered {
            syntax: "<length>".into(),
            inherits: true,
            initial: Some("1px".into()),
        }
    }

    pub fn snapshots_share_storage_and_survive_later_registrations() {
        let mut registry = PropertyRegistry::default();
        let empty = registry.snapshot();
        registry.register("--a", registration());
        let first = registry.snapshot();
        let same = registry.snapshot();
        assert!(Rc::ptr_eq(&first, &same));
        registry.register("--b", registration());
        let second = registry.snapshot();
        assert_eq!((empty.revision, empty.rules.len()), (0, 0));
        assert_eq!((first.revision, first.rules.len()), (1, 1));
        assert_eq!((second.revision, second.rules.len()), (2, 2));
        assert!(!first.rules.contains_key("--b"));
        assert!(!Rc::ptr_eq(&first, &second));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_snapshot_counts_how_its_registrations_inherit() {
        let reg = |inherits: bool, initial: Option<&str>| Registered {
            syntax: "*".into(),
            inherits,
            initial: initial.map(Into::into),
        };
        let mut registry = PropertyRegistry::default();
        registry.register("--a", reg(true, Some("1px")));
        registry.register("--b", reg(true, None));
        registry.register("--c", reg(false, Some("red")));
        registry.register("--a", reg(false, None));
        let snapshot = registry.snapshot();
        assert_eq!(snapshot.non_inherited, 1);
        assert_eq!(snapshot.inherited_initial, ["--a"]);
    }
}
