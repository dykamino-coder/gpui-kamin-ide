//! Пространство имён по умолчанию из `@namespace url(…)` без префикса (css-namespaces-3 §2.2):
//! действует на селекторы типа во время разбора таблицы.

use super::SelectorCx;

/// Тег-метка: `\u{1}h` — только XHTML, `\u{1}o` — только чужие элементы.
pub(crate) const MARK: char = '\u{1}';

/// Имя типа с меткой пространства по умолчанию (если оно объявлено).
pub(crate) fn mark_tag(name: String, cx: &SelectorCx) -> String {
    match cx.default_ns {
        Some(xhtml) => format!("{MARK}{}{name}", if xhtml { 'h' } else { 'o' }),
        None => name,
    }
}

/// Совпадает ли тег селектора (возможно, с меткой) с элементом.
pub(crate) fn tag_matches(want: &str, tag: &str, foreign: bool) -> bool {
    match want.strip_prefix(MARK) {
        Some(rest) => {
            let (ns, name) = rest.split_at(1);
            (ns == "o") == foreign && same_name(name, tag)
        }
        None => same_name(want, tag),
    }
}

/// The selector's name is stored lower-cased, while SVG keeps camelCase local
/// names (`foreignObject`, `clipPath`). Browsers match those against the
/// selector's source case (selectors-4 §type selectors; Blink keeps both
/// spellings), so a camelCase element is compared ASCII case-insensitively
/// here; all-lower-case names keep the exact comparison.
fn same_name(want: &str, tag: &str) -> bool {
    want == tag || (tag.bytes().any(|b| b.is_ascii_uppercase()) && want.eq_ignore_ascii_case(tag))
}

/// Аргумент `:is()`/`:where()` с меткой пространства по умолчанию:
/// пространство таблицы запоминается при разборе, чтобы селекторы типа
/// внутри (`*|*:is(div)`, `is-default-ns-001`) разбирались в нём же.
pub(crate) fn mark_arg(arg: &str, cx: &SelectorCx) -> String {
    match cx.default_ns {
        Some(xhtml) => format!("{MARK}{}{arg}", if xhtml { 'h' } else { 'o' }),
        None => arg.to_string(),
    }
}

/// Годные селекторы списка `:is()` (прощающий разбор, без псевдоэлементов).
pub(crate) fn is_args(arg: &str) -> Vec<crate::style::css::Selector> {
    let (ns, list) = match arg.strip_prefix(MARK) {
        Some(rest) if rest.len() > 1 => (Some(rest.starts_with('h')), &rest[1..]),
        _ => (None, arg),
    };
    let cx = SelectorCx::with_default_ns(ns);
    crate::style::css::split_selector_list(list)
        .into_iter()
        .filter_map(|one| crate::style::css::Selector::parse_in(one.trim(), &cx))
        .filter(|s| !s.has_pseudo_element())
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn lower_cased_selector_matches_camel_case_svg_tag() {
        assert!(super::tag_matches("foreignobject", "foreignObject", true));
        assert!(super::tag_matches("div", "div", false));
        assert!(!super::tag_matches("span", "div", false));
    }
}
