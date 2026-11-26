//! Class tokens and CSS comments as matching sees them: HTML splits a class
//! attribute on ASCII whitespace only, and a CSS comment produces no token.

use crate::dom::{Element, Node, parse};

fn color(html: &str, tag: &str) -> Vec<String> {
    fn collect<'a>(nodes: &'a [Node], tag: &str, out: &mut Vec<&'a Element>) {
        for n in nodes {
            if let Node::Element(e) = n {
                if e.tag == tag {
                    out.push(e);
                }
                collect(&e.children, tag, out);
            }
        }
    }
    let parsed = parse(html, "");
    let mut found = vec![];
    collect(&parsed.nodes, tag, &mut found);
    (found.iter())
        .map(|e| format!("{:?}", e.style.cascaded().inh.ui.color))
        .collect()
}

#[test]
fn a_no_break_space_does_not_separate_class_tokens() {
    // HTML §2.3.8: a set of space-separated tokens is split on ASCII
    // whitespace; U+00A0 belongs to the token `a\u{a0}b`.
    let html = |class: &str| {
        format!(
            "<!doctype html><style>.a{{color:red}}.a i{{color:red}}</style>\
             <p class=\"{class}\"><i>x</i></p>"
        )
    };
    let plain = "<!doctype html><p><i>x</i></p>";
    assert_eq!(
        color(&html("a\u{a0}b"), "p"),
        color(plain, "p"),
        "the element"
    );
    assert_eq!(
        color(&html("a\u{a0}b"), "i"),
        color(plain, "i"),
        "a descendant"
    );
    assert_ne!(color(&html("a b"), "p"), color(plain, "p"), "ASCII space");
}

#[test]
fn a_comment_between_compound_parts_is_not_a_combinator() {
    // CSS Syntax §4.3.2: a comment is consumed without a token, so
    // `div/**/.a` is the compound `div.a`, not `div .a`.
    let html = "<!doctype html><style>div/**/.a{color:red}</style>\
                <div class=a>x</div><div><p class=a>y</p></div>";
    let fresh = "<!doctype html><style>div.a{color:red}</style>\
                 <div class=a>x</div><div><p class=a>y</p></div>";
    assert_eq!(color(html, "div"), color(fresh, "div"));
    assert_eq!(color(html, "p"), color(fresh, "p"));
}
