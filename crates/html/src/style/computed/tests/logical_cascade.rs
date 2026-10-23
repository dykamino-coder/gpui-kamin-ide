//! Тесты: логическое и физическое свойство одной стороны или размера спорят
//! каскадом (css-logical-1 §4), а не порядком применения; общие слова и
//! подстановки у длинных свойств сокращений.

use crate::document::Document;
use crate::dom::{Element, Node};
use crate::style::values::value::Len;

fn find<'a>(nodes: &'a [Node], id: &str) -> Option<&'a Element> {
    nodes.iter().find_map(|n| match n {
        Node::Element(e) if e.attrs.iter().any(|(k, v)| k == "id" && v == id) => Some(e),
        Node::Element(e) => find(&e.children, id),
        _ => None,
    })
}

#[test]
fn stronger_declaration_wins_between_logical_and_physical_sizes() {
    // css-logical `cascading-001`.
    let doc = Document::builder(
        r#"<style>
        .hl { inline-size: 100px; block-size: 10px }
        #hl { inline-size: 100px; block-size: 10px }
        .hp { width: 100px; height: 10px }
        #hp { width: 100px; height: 10px }
        .vl { inline-size: 10px; block-size: 100px }
        #vl { inline-size: 10px; block-size: 100px }
        .vp { width: 10px; height: 100px }
        #vp { width: 10px; height: 100px }
        </style><body>
        <div id="vp" class="hl"></div><div id="vl" class="hp"></div>
        <div id="hp" class="vl"></div><div id="hl" class="vp"></div></body>"#,
        "",
    )
    .build();
    for (id, w, h) in [
        ("vp", 10.0, 100.0),
        ("vl", 10.0, 100.0),
        ("hp", 100.0, 10.0),
        ("hl", 100.0, 10.0),
    ] {
        let e = find(doc.nodes(), id).expect(id);
        assert_eq!(
            (e.style.cascaded().width, e.style.cascaded().height),
            (Some(Len::Px(w)), Some(Len::Px(h))),
            "{id}"
        );
    }
}

#[test]
fn author_physical_side_beats_user_agent_logical_side() {
    // `line-box-direction-vrl-019`: UA `padding-inline-start: 40px` of a list
    // lands on the top in `vertical-rl`; the author's `padding-top` wins.
    let doc = Document::builder(
        r#"<style>
        p { padding-left: 5px; margin-inline-start: 1px }
        #b { padding-inline-start: 9px }
        #c { margin-left: 7px }
        </style><body>
        <div style="writing-mode: vertical-rl"><ul id="a" style="padding-top: 0"></ul></div>
        <p id="b"></p><p id="c" style="margin-inline-start: 3px"></p></body>"#,
        "",
    )
    .build();
    let a = find(doc.nodes(), "a").expect("a");
    assert_eq!(a.style.cascaded().padding.top, Some(Len::Px(0.0)));
    let b = find(doc.nodes(), "b").expect("b");
    assert_eq!(b.style.cascaded().padding.left, Some(Len::Px(9.0)));
    let c = find(doc.nodes(), "c").expect("c");
    assert_eq!(c.style.cascaded().margin.left, Some(Len::Px(3.0)));
}

#[test]
fn wide_keywords_and_substitution_in_box_longhands() {
    let doc = Document::builder(
        r#"<style>
        #p { margin: 4px; padding: 2px; --a: 5px 6px; --bad: bogus }
        #a { margin-inline-start: inherit; padding: initial; top: unset }
        #b { margin: var(--a); padding: 3px; padding: var(--bad) }
        </style><body><div id="p"><div id="a"></div><div id="b"></div></div></body>"#,
        "",
    )
    .build();
    let a = find(doc.nodes(), "a").expect("a");
    assert!(
        a.style
            .cascaded()
            .inherit_longhands
            .contains(crate::style::css::LonghandId::MarginLeft)
    );
    assert_eq!(a.style.cascaded().padding.left, Some(Len::Px(0.0)));
    assert_eq!(a.style.cascaded().inset.top, Some(Len::Auto));
    let b = find(doc.nodes(), "b").expect("b");
    assert_eq!(b.style.cascaded().margin.top, Some(Len::Px(5.0)));
    assert_eq!(b.style.cascaded().margin.right, Some(Len::Px(6.0)));
    // Invalid after substitution: `unset`, not the earlier `padding: 3px`.
    assert_eq!(b.style.cascaded().padding.top, Some(Len::Px(0.0)));
}
