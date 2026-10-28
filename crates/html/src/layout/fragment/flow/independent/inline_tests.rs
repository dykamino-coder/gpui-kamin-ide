//! Буквица `initial-letter` — float записанных строк: копия остаётся нативной.

use super::{independent_copy, vertical_copy};
use crate::dom::{Element, Node};
use crate::style::computed::Computed;

fn sample(children: Vec<Node>, letter: bool) -> Element {
    let mut e = Element::synthetic("div", Computed::default(), Default::default(), children);
    if letter {
        let mut layer = Computed::default();
        layer.initial_letter = Some((3.0, 3));
        e.first_letter = Some(layer.into());
    }
    e
}

#[test]
fn initial_letter_keeps_the_native_copy() {
    let e = sample(vec![Node::Text("ABC ABC ABC".into())], true);
    assert!(independent_copy(&e));
    assert!(vertical_copy(&e, true));
    assert!(vertical_copy(&e, false));
}

#[test]
fn ruby_still_refuses_the_native_copy() {
    // The F2-6 refusal in `continuation.rs` stays until ruby line breaking is fixed.
    let ruby = Element::synthetic(
        "ruby",
        Computed::default(),
        Default::default(),
        vec![Node::Text("ABC".into())],
    );
    let e = sample(vec![Node::Element(ruby)], false);
    assert!(!independent_copy(&e));
    assert!(!vertical_copy(&e, true));
}
