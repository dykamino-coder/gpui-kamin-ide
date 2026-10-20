//! Parsed MATH data is cached by font source; page aliases are reset per document.

use crate::document::state::ValueCx;
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

pub(super) fn clear_aliases(vcx: ValueCx<'_>) {
    vcx.doc.font_math.families.borrow_mut().clear();
    vcx.doc.font_math.font_bytes.borrow_mut().clear();
}

pub(super) fn remember(source: &str, bytes: &[u8], vcx: ValueCx<'_>) {
    if Constants::parse(bytes).is_some() {
        vcx.doc
            .font_math
            .bytes
            .borrow_mut()
            .entry(source.into())
            .or_insert_with(|| bytes.into());
    }
    vcx.doc
        .font_math
        .sources
        .borrow_mut()
        .entry(source.into())
        .or_insert_with(|| Constants::parse(bytes).map(Arc::new));
}

pub(super) fn register(source: &str, family: &str, vcx: ValueCx<'_>) {
    if let Some(bytes) = vcx.doc.font_math.bytes.borrow().get(source).cloned() {
        vcx.doc
            .font_math
            .font_bytes
            .borrow_mut()
            .insert(family.to_ascii_lowercase(), bytes);
    }
    if let Some(Some(constants)) = vcx.doc.font_math.sources.borrow().get(source).cloned() {
        vcx.doc
            .font_math
            .families
            .borrow_mut()
            .insert(family.to_ascii_lowercase(), constants);
    }
}

pub(crate) fn font_bytes(family: &str, vcx: ValueCx<'_>) -> Option<Arc<[u8]>> {
    vcx.doc
        .font_math
        .font_bytes
        .borrow()
        .get(&family.to_ascii_lowercase())
        .cloned()
}

pub(crate) fn for_family(list: &str, vcx: ValueCx<'_>) -> Option<Arc<Constants>> {
    {
        let families = vcx.doc.font_math.families.borrow();
        families.get(&list.to_ascii_lowercase()).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_font_has_no_math_constants() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        remember("missing", b"not a font", vcx);
        register("missing", "Math", vcx);
        assert!(for_family("Math", vcx).is_none());
    }
    #[test]
    fn page_aliases_do_not_outlive_the_document() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let constants = Arc::new(Constants {
            script_scale: 0.8,
            script_script_scale: 0.6,
            values: [0.0; 51],
            degree_raise: 0.6,
        });
        vcx.doc
            .font_math
            .sources
            .borrow_mut()
            .insert("first-source".into(), Some(constants.clone()));
        register("first-source", "Page Math", vcx);
        assert!(Arc::ptr_eq(
            &for_family("Page Math", vcx).unwrap(),
            &constants
        ));
        clear_aliases(vcx);
        assert!(for_family("Page Math", vcx).is_none());
        register("first-source", "Fresh Name", vcx);
        assert!(Arc::ptr_eq(
            &for_family("fresh name", vcx).unwrap(),
            &constants
        ));
    }
    #[test]
    fn same_math_family_is_isolated_between_live_documents() {
        let first = crate::document::state::DocState::new(crate::style::css::Media::default());
        let second = crate::document::state::DocState::new(crate::style::css::Media::default());
        let a = first.values();
        let b = second.values();
        for (vcx, scale) in [(a, 0.8), (b, 0.5)] {
            vcx.doc.font_math.sources.borrow_mut().insert(
                "same-url".into(),
                Some(Arc::new(Constants {
                    script_scale: scale,
                    script_script_scale: 0.6,
                    values: [0.0; 51],
                    degree_raise: 0.6,
                })),
            );
            register("same-url", "Math", vcx);
        }
        assert_eq!(for_family("Math", a).unwrap().script_scale, 0.8);
        assert_eq!(for_family("Math", b).unwrap().script_scale, 0.5);
        clear_aliases(b);
        assert!(for_family("Math", b).is_none());
        assert_eq!(for_family("Math", a).unwrap().script_scale, 0.8);
    }
}
