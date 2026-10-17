//! Parsed MATH data is cached by font source; page aliases are reset per document.

#[path = "math_constants.rs"]
mod constants;
use constants::Constants;
use std::{cell::RefCell, collections::HashMap, sync::Arc};

thread_local! {
    static SOURCES: RefCell<HashMap<String, Option<Arc<Constants>>>> = RefCell::new(HashMap::new());
    static FAMILIES: RefCell<HashMap<String, Arc<Constants>>> = RefCell::new(HashMap::new());
    static BYTES: RefCell<HashMap<String, Arc<[u8]>>> = RefCell::new(HashMap::new());
    static FONT_BYTES: RefCell<HashMap<String, Arc<[u8]>>> = RefCell::new(HashMap::new());
}

pub(super) fn clear_aliases() {
    FAMILIES.with(|families| families.borrow_mut().clear());
    FONT_BYTES.with(|families| families.borrow_mut().clear());
}

pub(super) fn remember(source: &str, bytes: &[u8]) {
    if Constants::parse(bytes).is_some() {
        BYTES.with(|fonts| {
            fonts
                .borrow_mut()
                .entry(source.into())
                .or_insert_with(|| bytes.into());
        });
    }
    SOURCES.with(|sources| {
        sources
            .borrow_mut()
            .entry(source.into())
            .or_insert_with(|| Constants::parse(bytes).map(Arc::new));
    });
}

pub(super) fn register(source: &str, family: &str) {
    if let Some(bytes) = BYTES.with(|fonts| fonts.borrow().get(source).cloned()) {
        FONT_BYTES.with(|fonts| {
            fonts
                .borrow_mut()
                .insert(family.to_ascii_lowercase(), bytes);
        });
    }
    if let Some(Some(constants)) = SOURCES.with(|sources| sources.borrow().get(source).cloned()) {
        FAMILIES.with(|families| {
            families
                .borrow_mut()
                .insert(family.to_ascii_lowercase(), constants);
        });
    }
}

pub(crate) fn font_bytes(family: &str) -> Option<Arc<[u8]>> {
    FONT_BYTES.with(|fonts| fonts.borrow().get(&family.to_ascii_lowercase()).cloned())
}

pub(crate) fn for_family(list: &str) -> Option<Arc<Constants>> {
    FAMILIES.with(|families| {
        let families = families.borrow();
        families.get(&list.to_ascii_lowercase()).cloned()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_font_has_no_math_constants() {
        remember("missing", b"not a font");
        register("missing", "Math");
        assert!(for_family("Math").is_none());
    }
    #[test]
    fn page_aliases_do_not_outlive_the_document() {
        let constants = Arc::new(Constants {
            script_scale: 0.8,
            script_script_scale: 0.6,
            values: [0.0; 51],
            degree_raise: 0.6,
        });
        SOURCES.with(|sources| {
            sources
                .borrow_mut()
                .insert("first-source".into(), Some(constants.clone()))
        });
        register("first-source", "Page Math");
        assert!(Arc::ptr_eq(&for_family("Page Math").unwrap(), &constants));
        clear_aliases();
        assert!(for_family("Page Math").is_none());
        register("first-source", "Fresh Name");
        assert!(Arc::ptr_eq(&for_family("fresh name").unwrap(), &constants));
    }
}
