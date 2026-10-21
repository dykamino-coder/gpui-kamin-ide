//! Дескрипторы шрифта и списки источников используют общие токенизаторы CSS-правил и значений.

use crate::document::state::ValueCx;
use crate::style::css;

pub(super) fn faces(css: &str, vcx: ValueCx<'_>) -> Vec<String> {
    // `@font-face` под `@media` решает носитель текущего документа.
    css::resource_rules::resources(css, vcx.doc.media(), vcx)
        .into_iter()
        .filter(|rule| rule.name == "font-face")
        .map(|rule| rule.body)
        .collect()
}

pub(super) fn declaration(block: &str, name: &str) -> Option<String> {
    css::resource_rules::descriptor(block, name)
}

/// Select the first supported file source; local() is resolved by system fallback.
pub(super) fn source(block: &str) -> Option<String> {
    let src = declaration(block, "src")?;
    let mut fallback = None;
    for part in css::split_top_level(&src, ',') {
        let part = part.trim();
        if !css::at_url(part) {
            continue;
        }
        let Some(path) = crate::style::computed::parse_url(part) else {
            continue;
        };
        let path = css::unescape(&path);
        let known = matches!(
            std::path::Path::new(&path)
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_ascii_lowercase)
                .as_deref(),
            Some("ttf" | "otf" | "woff" | "woff2")
        );
        if known {
            return Some(path);
        }
        fallback.get_or_insert(path);
    }
    fallback
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_list_respects_strings_escapes_and_function_tokens() {
        assert_eq!(
            source(r#"src: local("url(fake.ttf)"), URL("font)name.ttf")"#).as_deref(),
            Some("font)name.ttf")
        );
        assert_eq!(
            source(r#"src: url('a;b}c.ttf'); src: url('last.ttf')"#).as_deref(),
            Some("last.ttf")
        );
        assert!(source(r#"src: "url(fake.ttf)""#).is_none());
    }
}
