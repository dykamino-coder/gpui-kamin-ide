//! Буквица `initial-letter` и ruby — содержимое записанных строк: копия остаётся нативной.

use super::{independent_copy, vertical_copy};
use crate::dom::{Element, Node};
use crate::style::computed::Computed;

fn sample(children: Vec<Node>, letter: bool) -> Element {
    let mut e = Element::synthetic("div", Computed::default(), Default::default(), children);
    if letter {
        let mut layer = Computed::default();
        layer.reset_text_box.initial_letter = Some((3.0, 3));
        e.first_letter = Some(layer.into());
    }
    e
}

#[test]
fn initial_letter_and_ruby_keep_the_native_copy() {
    let text = || vec![Node::Text("ABC ABC ABC".into())];
    let ruby = Element::synthetic("ruby", Computed::default(), Default::default(), text());
    for e in [
        sample(text(), true),
        sample(vec![Node::Element(ruby)], false),
    ] {
        assert!(independent_copy(&e), "{}", e.tag);
        assert!(vertical_copy(&e, true));
        assert!(vertical_copy(&e, false));
    }
}
