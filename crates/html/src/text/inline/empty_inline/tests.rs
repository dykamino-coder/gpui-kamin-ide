//! Факты текста не меняют участие скрытых коробок и отделяются при правке.

use super::*;
use crate::layout::cx::subtree::SubtreeFacts;
use crate::layout::table::anon::anon_element;

#[test]
fn prepared_text_preserves_in_flow_visibility_and_css_whitespace() {
    for (display, float, position, visible) in [
        (None, None, None, true),
        (Some(Display::Contents), None, None, true),
        (Some(Display::None), None, None, false),
        (None, Some(1), None, false),
        (None, None, Some(Position::Absolute), false),
        (None, None, Some(Position::Fixed), false),
    ] {
        for (text, significant) in [(" \t\r\n", false), ("\u{a0}", true), ("x", true)] {
            let mut child = anon_element("span", vec![Node::Text(text.into())]);
            let style = child.style.used_mut();
            style.reset_box_layout.display = display;
            style.reset_box_layout.float = float;
            style.reset_positioned.position = position;
            let root = anon_element("span", vec![Node::Element(child)]);
            let expected = visible && significant;
            assert_eq!(has_text(&root.children), expected);
            SubtreeFacts::default().breaks_absent(&root);
            assert_eq!(has_text(&root.children), expected);
        }
    }
}

#[test]
fn a_text_edit_detaches_the_facts_from_the_previous_revision() {
    let mut root = anon_element(
        "span",
        vec![Node::Element(anon_element(
            "span",
            vec![Node::Text("old".into())],
        ))],
    );
    SubtreeFacts::default().breaks_absent(&root);
    let old = root.clone();
    let Node::Element(child) = &mut root.children[0] else {
        unreachable!()
    };
    child.children[0] = Node::Text(" \t".into());
    assert!(!has_text(&root.children));
    assert!(has_text(&old.children));
    SubtreeFacts::default().breaks_absent(&root);
    assert!(!has_text(&root.children));
    assert!(has_text(&old.children));
}
