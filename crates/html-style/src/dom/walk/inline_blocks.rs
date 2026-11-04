//! Разобранные блоки инлайн-стиля одного обхода документа (Gecko хранит
//! разобранный блок `style=""` в значении атрибута, Blink кэширует разбор
//! по тексту). Разбор — функция текста атрибута и контекста значений
//! (окружение кэша каскада: корень, шрифты, viewport, media, схема); у
//! повторяющихся `style=""` он выполняется один раз, а каскад получает общий
//! `Rc` блока.

use crate::budget::admission::Admission;
use crate::style::cascade::sharing::Environment;
use crate::style::css::{DeclBlock, parse_decls_in};
use crate::values::ValueCx;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Разных текстов в памяти; при переполнении таблица начинается заново.
const CAPACITY: usize = 256;

struct Entry {
    environment: Environment,
    block: Rc<DeclBlock>,
    _lease: Option<crate::budget::LiveReservation>,
}

#[derive(Default)]
pub(super) struct InlineBlocks {
    entries: RefCell<HashMap<String, Entry>>,
}

impl InlineBlocks {
    /// Блок атрибута `style` элемента (пустой без атрибута). Разбор, записавший
    /// наблюдение в журнал документа, не запоминается: повтор записал бы его
    /// ещё раз.
    pub fn block(&self, attrs: &[(String, String)], vcx: ValueCx<'_>) -> Rc<DeclBlock> {
        let Some((_, raw)) = attrs.iter().find(|(k, _)| k == "style") else {
            return Rc::default();
        };
        if let Some(e) = self.entries.borrow().get(raw.as_str())
            && e.environment.matches(vcx)
        {
            crate::work::add!(InlineStyleParseHits, 1);
            return e.block.clone();
        }
        let observed = vcx.doc.diagnostic_observations();
        let block = Rc::new(parse_decls_in(raw, vcx));
        if observed != vcx.doc.diagnostic_observations() {
            return block;
        }
        let bytes = size_of::<Entry>()
            + raw.len()
            + block
                .0
                .iter()
                .map(|d| size_of_val(d) + d.name.len() + d.value.len())
                .sum::<usize>();
        let Ok(lease) = Admission::new(vcx.doc.revision_budget()).storage(bytes) else {
            return block;
        };
        let mut entries = self.entries.borrow_mut();
        if entries.len() == CAPACITY {
            entries.clear();
        }
        entries.insert(
            raw.clone(),
            Entry {
                environment: Environment::new(vcx),
                block: block.clone(),
                _lease: lease,
            },
        );
        block
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_texts_share_one_parsed_block() {
        let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
        let vcx = doc.values();
        let cache = InlineBlocks::default();
        let a = vec![("style".to_string(), "color:red;width:5px".to_string())];
        let b = vec![
            ("class".to_string(), "x".to_string()),
            ("style".to_string(), "color:red;width:5px".to_string()),
        ];
        let first = cache.block(&a, vcx);
        assert!(Rc::ptr_eq(&first, &cache.block(&b, vcx)));
        assert_eq!(*first, parse_decls_in("color:red;width:5px", vcx));
        let other = vec![("style".to_string(), "color:blue".to_string())];
        assert!(!Rc::ptr_eq(&first, &cache.block(&other, vcx)));
        assert!(cache.block(&[], vcx).is_empty());
    }

    #[test]
    fn a_different_color_scheme_parses_again() {
        let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
        let vcx = doc.values();
        let cache = InlineBlocks::default();
        let a = vec![(
            "style".to_string(),
            "color:light-dark(red,blue)".to_string(),
        )];
        let light = cache.block(&a, vcx);
        let dark = vcx.with_scheme(crate::style::values::value::Scheme::new(true));
        let again = cache.block(&a, dark);
        assert_eq!(*light, *again);
        assert!(Rc::ptr_eq(&again, &cache.block(&a, dark)));
    }
}
