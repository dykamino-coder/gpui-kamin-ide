//! Selector capability queries follow the parser used by the actual stylesheet cascade.

use super::supports;

#[test]
fn selector_queries_use_the_stylesheet_pseudo_grammar() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    for selector in [
        "input::-webkit-slider-runnable-track",
        "div::unknown-pseudo",
        "::picker",
        "::picker()",
        "::picker(foo)",
        "::picker(select, select)",
        "::picker(select foo)",
        "::picker-icon(foo)",
        "::-webkit-slider-thumb(foo)",
        "::details-content(foo)",
        "select:picker(select)",
        "select:picker-icon",
        "input:-webkit-slider-thumb",
        "details:details-content",
        "div::hover",
        "div:::before",
        "details::details-content.foo",
        "details::details-content[data-x]",
        "details::details-content:hover.foo",
        "details::details-content::before::after",
        "details::details-content::before > p",
    ] {
        assert!(
            !supports(&format!("selector({selector})"), vcx),
            "{selector}"
        );
        assert!(
            crate::style::css::Selector::parse(selector).is_none(),
            "{selector}"
        );
    }
    for selector in [
        "::picker(select)",
        "select::picker(select select)",
        "select::PICKER(SELECT)",
        r"select::picker(s\65 lect)",
        "select::picker-icon",
        "input::-webkit-slider-thumb",
        "input::details-content::before",
        "details::details-content::after",
        "details::details-content:hover::before",
        "div:before",
        "div:after",
        "div:first-line",
        "div:first-letter",
        "div::before",
        "details::details-content",
        ".x:hover",
    ] {
        assert!(
            supports(&format!("selector({selector})"), vcx),
            "{selector}"
        );
        let rules =
            crate::style::css::parse_stylesheet(&format!("{selector}, .control {{color: green}}"));
        assert_eq!(rules.len(), 2, "{selector}");
    }
    assert!(supports("selector(&)", vcx));
    assert!(supports("selector(& > p)", vcx));
}

#[test]
fn sub_pseudo_elements_remain_in_the_selector_chain() {
    let sel = crate::style::css::Selector::parse("details::details-content::before").unwrap();
    assert_eq!(sel.pseudo.as_deref(), Some("before"));
    assert_eq!(sel.also, ["details-content"]);
    assert!(sel.has_pseudo_element());
}

#[test]
fn pseudo_rules_do_not_style_the_originating_element() {
    use crate::dom::Node;
    use crate::style::values::value::Len;

    fn check(nodes: &[Node]) -> usize {
        let mut found = 0;
        for node in nodes {
            if let Node::Element(e) = node {
                if e.tag == "div" {
                    assert_eq!(e.style.width, Some(Len::Px(17.0)));
                    found += 1;
                }
                assert_ne!(
                    e.tag, "::before",
                    "sub-pseudo must not become host ::before"
                );
                found += check(&e.children);
            }
        }
        found
    }

    let tree = crate::dom::parse(
        "<!doctype html><div>x</div>",
        r#"div { width:17px }
        div::picker(select), div::picker-icon, div::-webkit-slider-thumb,
        div::details-content::before { width:999px; content:"bad" }"#,
    );
    assert_eq!(check(&tree), 1);
}

#[test]
fn nesting_and_pseudo_names_inside_attribute_strings_are_preserved() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    for selector in [
        r#"[data-value="&"]"#,
        r#"[data-value="::picker(select)"]"#,
        r#"[data-value="::-webkit-slider-thumb"]"#,
        r#"[data-value="::picker-icon"]"#,
    ] {
        assert!(
            supports(&format!("selector({selector})"), vcx),
            "{selector}"
        );
    }
    assert!(!supports("selector(div, span)", vcx));
}
