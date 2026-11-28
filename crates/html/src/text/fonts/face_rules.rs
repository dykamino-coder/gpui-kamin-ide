//! Дескрипторы шрифта и списки источников используют общие токенизаторы CSS-правил и значений.

use crate::style::css;
use crate::values::ValueCx;

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

/// CSS Fonts §4.3: try URL sources in order until one activates successfully.
/// A filename extension does not establish support or reorder the list.
pub(super) fn try_source<T, E>(
    block: &str,
    mut activate: impl FnMut(String) -> Result<Option<T>, E>,
) -> Result<Option<T>, E> {
    let Some(src) = declaration(block, "src") else {
        return Ok(None);
    };
    for part in css::split_top_level(&src, ',') {
        let part = part.trim();
        if !css::at_url(part) {
            continue;
        }
        let Some(path) = crate::style::computed::parse_url(part) else {
            continue;
        };
        let path = css::unescape(&path);
        if let Some(active) = activate(path)? {
            return Ok(Some(active));
        }
    }
    Ok(None)
}

#[cfg(test)]
pub(super) fn source(block: &str) -> Option<String> {
    try_source::<_, std::convert::Infallible>(block, |path| Ok(Some(path))).unwrap()
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
