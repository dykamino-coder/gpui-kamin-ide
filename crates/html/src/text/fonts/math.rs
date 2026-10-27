//! Данные MATH кэшируются по источнику шрифта; алиасы страницы сбрасываются на документ.

use crate::text::text_fonts::TextFonts;
#[path = "math_constants.rs"]
mod constants;
use constants::Constants;
use std::{cell::RefCell, collections::HashMap, sync::Arc};

#[derive(Default)]
pub(crate) struct MathFonts {
    sources: RefCell<HashMap<String, Option<Arc<Constants>>>>,
    families: RefCell<HashMap<String, Arc<Constants>>>,
    bytes: RefCell<HashMap<String, Arc<[u8]>>>,
    font_bytes: RefCell<HashMap<String, Arc<[u8]>>>,
}

pub(super) fn clear_aliases(fonts: &TextFonts) {
    fonts.math.families.borrow_mut().clear();
    fonts.math.font_bytes.borrow_mut().clear();
}

pub(super) fn remember(source: &str, bytes: &[u8], fonts: &TextFonts) {
    if Constants::parse(bytes).is_some() {
        fonts
            .math
            .bytes
            .borrow_mut()
            .entry(source.into())
            .or_insert_with(|| bytes.into());
    }
    fonts
        .math
        .sources
        .borrow_mut()
        .entry(source.into())
        .or_insert_with(|| Constants::parse(bytes).map(Arc::new));
}

pub(super) fn register(source: &str, family: &str, fonts: &TextFonts) {
    if let Some(bytes) = fonts.math.bytes.borrow().get(source).cloned() {
        fonts
            .math
            .font_bytes
            .borrow_mut()
            .insert(family.to_ascii_lowercase(), bytes);
    }
    if let Some(Some(constants)) = fonts.math.sources.borrow().get(source).cloned() {
        fonts
            .math
            .families
            .borrow_mut()
            .insert(family.to_ascii_lowercase(), constants);
    }
}

pub(crate) fn font_bytes(family: &str, fonts: &TextFonts) -> Option<Arc<[u8]>> {
    fonts
        .math
        .font_bytes
        .borrow()
        .get(&family.to_ascii_lowercase())
        .cloned()
}

pub(crate) fn for_family(list: &str, fonts: &TextFonts) -> Option<Arc<Constants>> {
    {
        let families = fonts.math.families.borrow();
        families.get(&list.to_ascii_lowercase()).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_font_has_no_math_constants() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let fonts = &*doc.text;
        remember("missing", b"not a font", fonts);
        register("missing", "Math", fonts);
        assert!(for_family("Math", fonts).is_none());
    }
    #[test]
    fn page_aliases_do_not_outlive_the_document() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let fonts = &*doc.text;
        let constants = Arc::new(Constants {
            script_scale: 0.8,
            script_script_scale: 0.6,
            values: [0.0; 51],
            degree_raise: 0.6,
        });
        fonts
            .math
            .sources
            .borrow_mut()
            .insert("first-source".into(), Some(constants.clone()));
        register("first-source", "Page Math", fonts);
        assert!(Arc::ptr_eq(
            &for_family("Page Math", fonts).unwrap(),
            &constants
        ));
        clear_aliases(fonts);
        assert!(for_family("Page Math", fonts).is_none());
        register("first-source", "Fresh Name", fonts);
        assert!(Arc::ptr_eq(
            &for_family("fresh name", fonts).unwrap(),
            &constants
        ));
    }
    #[test]
    fn same_math_family_is_isolated_between_live_documents() {
        let first = crate::document::state::DocState::new(crate::style::css::Media::default());
        let second = crate::document::state::DocState::new(crate::style::css::Media::default());
        let a = &*first.text;
        let b = &*second.text;
        for (fonts, scale) in [(a, 0.8), (b, 0.5)] {
            fonts.math.sources.borrow_mut().insert(
                "same-url".into(),
                Some(Arc::new(Constants {
                    script_scale: scale,
                    script_script_scale: 0.6,
                    values: [0.0; 51],
                    degree_raise: 0.6,
                })),
            );
            register("same-url", "Math", fonts);
        }
        assert_eq!(for_family("Math", a).unwrap().script_scale, 0.8);
        assert_eq!(for_family("Math", b).unwrap().script_scale, 0.5);
        clear_aliases(b);
        assert!(for_family("Math", b).is_none());
        assert_eq!(for_family("Math", a).unwrap().script_scale, 0.8);
    }
}
