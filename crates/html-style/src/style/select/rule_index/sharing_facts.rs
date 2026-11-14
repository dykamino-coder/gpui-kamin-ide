//! Факты правила для разделения стиля между элементами (Stylo «revalidation
//! selectors», Blink style sharing): устойчив ли селектор и зависят ли значения
//! от места элемента среди братьев.
//!
//! Устойчивый селектор состоит только из тегов, `*`, id, классов и атрибутных
//! условий, соединённых комбинаторами потомка и ребёнка. Его исход — функция
//! тега, атрибутов и цепочки предков; у элементов с равными атрибутами и
//! равнозначными цепочками он совпадает. Любой псевдокласс или псевдоэлемент
//! (структурные `:nth-*`, `:empty`, `:has()`, состояния) и соседние
//! комбинаторы `+`/`~` в любом компаунде делают правило перепроверяемым: его
//! исход сравнивается заново на каждом элементе.

use crate::style::css::{Rule, Selector};

/// Решения одного правила для разделения стиля между элементами.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SharingFacts {
    /// Исход правила нужно перепроверить на элементе, берущем чужой стиль.
    pub revalidate: bool,
    /// Значение может читать место элемента (`sibling-index()`,
    /// `sibling-count()`) либо скрывает имя функции экранированием.
    pub positional: bool,
    /// Селектор читает состояние ссылки в любом компаунде.
    pub link_state: bool,
}

impl SharingFacts {
    pub fn of(rule: &Rule) -> Self {
        Self {
            revalidate: !stable(&rule.sel),
            link_state: mentions_link_state(&rule.sel),
            positional: rule.decls.iter().any(|d| positional_text(&d.value)),
        }
    }
}

fn mentions_link_state(sel: &Selector) -> bool {
    sel.pseudo.iter().chain(&sel.also).any(|p| {
        let p = p.to_ascii_lowercase();
        p.contains("link") || p.contains("visited")
    }) || sel
        .prev
        .as_deref()
        .is_some_and(|(s, _)| mentions_link_state(s))
        || sel
            .ancestor
            .as_deref()
            .is_some_and(|(s, _)| mentions_link_state(s))
}

fn stable(sel: &Selector) -> bool {
    sel.pseudo.is_none()
        && sel.also.is_empty()
        && sel.arguments.is_empty()
        && sel.prev.is_none()
        && sel.ancestor.as_deref().is_none_or(|(a, _)| stable(a))
}

/// Текст значения, способный зависеть от места элемента среди братьев
/// (css-values-5 §tree-counting): `sibling-index()`/`sibling-count()` в
/// любом регистре; экранирование могло бы скрыть имя функции.
pub fn positional_text(value: &str) -> bool {
    value.contains('\\')
        || value
            .as_bytes()
            .windows(8)
            .any(|w| w.eq_ignore_ascii_case(b"sibling-"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::css::{LinkMatch, Origin};

    fn rule(sel: &str, decls: &str) -> Rule {
        Rule {
            sel: Selector::parse(sel).unwrap(),
            decls: crate::style::css::parse_decls(decls),
            order: 0,
            origin: Origin::Author,
            layer: vec![u32::MAX],
            link: LinkMatch::Any,
        }
    }

    #[test]
    fn type_class_id_attribute_and_ancestor_chains_are_stable() {
        for sel in [
            "div",
            "*",
            ".a",
            "#x",
            "[href]",
            "ul > li.a",
            "table td span",
            "a[href^='#']",
        ] {
            assert!(
                !SharingFacts::of(&rule(sel, "color:red")).revalidate,
                "{sel}"
            );
        }
    }

    #[test]
    fn pseudo_classes_and_sibling_combinators_anywhere_revalidate() {
        for sel in [
            ".row:nth-child(2n)",
            "tr:nth-child(2n) td",
            "p:empty",
            "a:hover",
            "p::before",
            ".a + .b",
            ".a ~ .b .c",
            "div:has(.x) span",
            ":is(.a) .b",
            "li:first-child",
        ] {
            assert!(
                SharingFacts::of(&rule(sel, "color:red")).revalidate,
                "{sel}"
            );
        }
    }

    #[test]
    fn sibling_functions_and_escapes_are_positional() {
        assert!(SharingFacts::of(&rule("div", "width:calc(10px * sibling-index())")).positional);
        assert!(SharingFacts::of(&rule("div", "order:SIBLING-COUNT()")).positional);
        assert!(SharingFacts::of(&rule("div", "--n:sibling-index()")).positional);
        assert!(!SharingFacts::of(&rule("div", "color:red;width:calc(1px + 2px)")).positional);
    }
}
