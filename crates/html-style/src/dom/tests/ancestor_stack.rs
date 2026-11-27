//! Тесты: селекторы предков работают через соседние поддеревья и сгенерированное содержимое.

use super::tree_parsing::{child_colors, pseudo_texts};
use crate::style::values::value::Color;

#[test]
fn completed_subtrees_do_not_leak_ancestors_into_siblings() {
    let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
    let vcx = doc.values();
    let html = r#"<style>
        #box > section > span { color: green; }
        #box > span { color: blue; }
        section span::before { content: 'inside'; }
        #box > span::before { content: 'outside'; }
        </style><div id=box><section><span>A</span></section><span>B</span></div>"#;
    let colors = child_colors(html);
    assert_eq!(colors[1], Color::parse("blue", vcx));
    assert_eq!(pseudo_texts(html), ["inside", "outside"]);
}

#[test]
fn unfocused_ancestors_do_not_activate_descendants_or_generated_content() {
    let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
    let vcx = doc.values();
    for state in ["hover", "active", "focus", "focus-visible", "focus-within"] {
        let html = format!(
            "<style>span{{color:green}} section:{state} span{{color:red}} \
             section:{state} span::after{{content:'unexpected'}} \
             </style><section id=box><span>item</span></section>"
        );
        assert_eq!(child_colors(&html), [Color::parse("green", vcx)], "{state}");
        assert!(pseudo_texts(&html).is_empty(), "{state}");
    }
}

#[test]
fn ancestor_language_and_direction_use_the_subject_predicate() {
    let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
    let vcx = doc.values();
    let html = "<style>span{color:green} section:lang(fr) span,\
                section:dir(rtl) span{color:red}</style>\
                <section id=box lang=en dir=ltr><span>item</span></section>";
    assert_eq!(child_colors(html), [Color::parse("green", vcx)]);
}

#[test]
fn ancestor_predicates_preserve_inherited_and_functional_matches() {
    let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
    let vcx = doc.values();
    for selector in [
        "section:lang(fr)",
        "section:dir(rtl)",
        "section:not(:focus-within)",
        "section:is(:lang(fr), :lang(en))",
    ] {
        let html = format!(
            "<style>span{{color:red}} {selector} span{{color:green}}</style>\
             <div lang=fr dir=rtl><section id=box><span>item</span></section></div>"
        );
        assert_eq!(
            child_colors(&html),
            [Color::parse("green", vcx)],
            "{selector}"
        );
    }
}
