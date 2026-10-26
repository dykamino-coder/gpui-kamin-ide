//! Неизменяемые снимки @property документа (CSS Properties & Values §3).

use crate::style::css::Registered;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone, Default)]
pub(crate) struct PropertySnapshot {
    pub revision: u64,
    pub rules: HashMap<String, Registered>,
}

#[derive(Default)]
pub(crate) struct PropertyRegistry {
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
        next.rules.insert(name.to_string(), registration);
        next.revision += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registration() -> Registered {
        Registered {
            syntax: "<length>".into(),
            inherits: true,
            initial: Some("1px".into()),
        }
    }

    #[test]
    fn snapshots_share_storage_and_survive_later_registrations() {
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
