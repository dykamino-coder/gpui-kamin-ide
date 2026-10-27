//! Независимые CSS-входы: допуск предшествует AST и вычислению запроса.

use super::{DeclBlock, Keyframes, Media, Selector};
use crate::{css_syntax::admission, dom::outcome::DocumentFailure};
use std::collections::HashMap;

impl Selector {
    /// Синтаксический промах — `None`, исчерпание допуска — типизированный отказ.
    pub fn try_parse(raw: &str) -> Result<Option<Self>, DocumentFailure> {
        let admitted = admission::standalone(raw)?;
        admission::selector_depth::measure(raw, &admitted)?;
        admitted.consume(crate::budget::BudgetKind::CssSelectors, 1)?;
        Ok(Self::parse(raw))
    }
}

/// Декларации независимого входа допускаются до раскрытия сокращений.
pub fn try_parse_decls(raw: &str) -> Result<DeclBlock, DocumentFailure> {
    let _admitted = admission::standalone(raw)?;
    Ok(super::parse_decls(raw))
}

/// Отдельный разбор кадров использует те же defaults и типизированный отказ.
pub fn try_parse_keyframes(
    css: &str,
    media: Media,
) -> Result<HashMap<String, Keyframes>, DocumentFailure> {
    let _admitted = admission::standalone(css)?;
    let doc = crate::document::state::DocState::new(media);
    Ok(super::parse_keyframes_in(css, Some(media), doc.values()))
}

impl Media {
    /// Отдельный медиазапрос допускается до выделения и обхода его грамматики.
    pub fn try_matches(&self, query: &str) -> Result<bool, DocumentFailure> {
        let _admitted = admission::standalone(query)?;
        let doc = crate::document::state::DocState::new(*self);
        Ok(self.matches(query, doc.values()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_inputs_distinguish_syntax_miss_and_resource_failure() {
        assert!(Selector::try_parse("").unwrap().is_none());
        assert!(Selector::try_parse(".a > .b").unwrap().is_some());
        assert!(Selector::try_parse(&"p ".repeat(10_000)).is_err());
        let chain = format!("{} {{ color:red }}", "p > ".repeat(10_000) + "p");
        assert!(super::super::parse_stylesheet(&chain).is_err());
        let deep = "(".repeat(10_000);
        assert!(try_parse_decls(&deep).is_err());
        assert!(try_parse_keyframes(&deep, Media::default()).is_err());
        assert!(Media::default().try_matches(&deep).is_err());
        assert_eq!(
            try_parse_decls("color: red").unwrap().last("color"),
            Some("red")
        );
        assert!(Media::default().try_matches("@media screen").unwrap());
    }
}
