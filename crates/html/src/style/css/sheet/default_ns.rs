//! Пространство имён по умолчанию из `@namespace url(…)` без префикса (css-namespaces-3 §2.2):
//! действует на селекторы типа во время разбора таблицы.

use super::SelectorCx;
use crate::style::css::TypeNamespace;

/// Ограничение пространства и имя проверяются независимо, без декодирования имени.
pub(crate) fn tag_matches(want: &str, ns: TypeNamespace, tag: &str, foreign: bool) -> bool {
    let namespace_matches = match ns {
        TypeNamespace::Any => true,
        TypeNamespace::Xhtml => !foreign,
        TypeNamespace::Foreign => foreign,
    };
    namespace_matches && same_name(want, tag)
}

/// The selector's name is stored lower-cased, while SVG keeps camelCase local
/// names (`foreignObject`, `clipPath`). Browsers match those against the
/// selector's source case (selectors-4 §type selectors; Blink keeps both
/// spellings), so a camelCase element is compared ASCII case-insensitively
/// here; all-lower-case names keep the exact comparison.
fn same_name(want: &str, tag: &str) -> bool {
    want == tag || (tag.bytes().any(|b| b.is_ascii_uppercase()) && want.eq_ignore_ascii_case(tag))
}

/// Годные селекторы списка `:is()` с отдельным контекстом namespace.
pub(crate) fn is_args(arg: &str, namespace: TypeNamespace) -> Vec<crate::style::css::Selector> {
    let ns = match namespace {
        TypeNamespace::Any => None,
        TypeNamespace::Xhtml => Some(true),
        TypeNamespace::Foreign => Some(false),
    };
    let cx = SelectorCx::with_default_ns(ns);
    crate::style::css::split_selector_list(arg)
        .into_iter()
        .filter_map(|one| crate::style::css::Selector::parse_in(one.trim(), &cx))
        .filter(|s| !s.has_pseudo_element())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::TypeNamespace::Any;
    #[test]
    fn lower_cased_selector_matches_camel_case_svg_tag() {
        assert!(super::tag_matches(
            "foreignobject",
            Any,
            "foreignObject",
            true
        ));
        assert!(super::tag_matches("div", Any, "div", false));
        assert!(!super::tag_matches("span", Any, "div", false));
    }

    #[test]
    fn f01_tag_matches_bare_mark_does_not_panic() {
        let r = std::panic::catch_unwind(|| {
            crate::style::css::default_ns::tag_matches("\u{1}", Any, "div", false)
        });
        assert!(r.is_ok(), "tag_matches(\"\\u{{1}}\") паникует");
    }

    #[test]
    fn f01_tag_matches_mark_then_multibyte_does_not_panic() {
        let r = std::panic::catch_unwind(|| {
            crate::style::css::default_ns::tag_matches("\u{1}é", Any, "div", false)
        });
        assert!(r.is_ok(), "split_at(1) режет UTF-8");
    }

    #[test]
    fn f01_escaped_control_selector_in_document_does_not_panic() {
        let r = std::panic::catch_unwind(|| {
            crate::document::Document::builder(
                "<style>\\1 {color:red}</style><div id=d>x</div>",
                "",
            )
            .build()
            .nodes()
            .len()
        });
        assert!(r.is_ok(), "Document::new паникует на селекторе `\\1`");
    }

    #[test]
    fn type_namespace_is_separate_from_escaped_name() {
        use crate::style::css::{Selector, TypeNamespace};
        for (default, expected) in [
            (None, TypeNamespace::Any),
            (Some(true), TypeNamespace::Xhtml),
            (Some(false), TypeNamespace::Foreign),
        ] {
            let cx = super::SelectorCx::with_default_ns(default);
            for (source, name) in [
                (r"\1", "\u{1}"),
                (r"\1 é", "\u{1}é"),
                (r"\1 hdiv", "\u{1}hdiv"),
                ("DIV", "div"),
            ] {
                let sel = Selector::parse_in(source, &cx).expect("valid escaped identifier");
                assert_eq!(sel.tag.as_deref(), Some(name));
                assert_eq!(sel.tag_namespace, expected);
                for foreign in [false, true] {
                    assert_eq!(
                        super::tag_matches(name, expected, name, foreign),
                        default.is_none_or(|html| html != foreign)
                    );
                    assert!(!super::tag_matches(name, expected, "span", foreign));
                    if name.starts_with('\u{1}') {
                        assert!(!super::tag_matches(name, expected, "div", foreign));
                    }
                }
            }
            let explicit = Selector::parse_in("*|div", &cx).expect("explicit namespace");
            assert_eq!(explicit.tag_namespace, TypeNamespace::Any);
        }
    }

    #[test]
    fn delayed_arguments_keep_namespace_separate_from_user_text() {
        use crate::style::css::{Selector, TypeNamespace};
        for (default, expected) in [
            (None, TypeNamespace::Any),
            (Some(true), TypeNamespace::Xhtml),
            (Some(false), TypeNamespace::Foreign),
        ] {
            let cx = super::SelectorCx::with_default_ns(default);
            let outer = Selector::parse_in(r":is(\1 é, DIV):where(:is(div))", &cx).unwrap();
            assert_eq!(outer.argument_namespace, expected);
            assert_eq!(outer.also[0], r"is(\1 é, DIV)");
            let parsed = super::is_args(r"\1 é, DIV", outer.argument_namespace);
            assert_eq!(parsed.len(), 2);
            assert_eq!(parsed[0].tag.as_deref(), Some("\u{1}é"));
            assert_eq!(parsed[1].tag.as_deref(), Some("div"));
            assert!(parsed.iter().all(|s| s.tag_namespace == expected));
            let nested = super::is_args(":is(div)", outer.argument_namespace);
            assert_eq!(nested[0].argument_namespace, expected);
            for arg in ["\u{1}", "\u{1}é", "\u{1}h", "\u{1}o", "\u{1}💡"] {
                let _ = super::is_args(arg, expected);
            }
        }
    }
}
