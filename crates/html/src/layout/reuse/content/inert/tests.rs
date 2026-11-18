//! Subtree verdicts of a build: linear in the frame however deep the containers nest.

use super::*;
use crate::dom::Marks;
use crate::style::computed::Computed;
use std::cell::Cell;

fn el(tag: &str, children: Vec<Node>) -> Node {
    Node::Element(Element::synthetic(
        tag,
        Computed::default(),
        Marks::default(),
        children,
    ))
}

/// A chain of `depth` nested `div`s around `leaf`: the node of each level,
/// outermost first.
fn chain(depth: usize, leaf: Node) -> Vec<Node> {
    let mut levels = vec![leaf];
    for _ in 0..depth {
        let inner = levels.last().cloned().expect("a level");
        levels.push(el("div", vec![inner]));
    }
    levels.reverse();
    levels
}

#[test]
fn every_container_of_a_deep_chain_asks_once_per_element() {
    let levels = chain(300, el("p", vec![]));
    let lists = InertLists::default();
    let visits = Cell::new(0usize);
    let own = |_: &Element| {
        visits.set(visits.get() + 1);
        true
    };
    // As the build asks: each container for its content, outermost first.
    for level in &levels {
        assert!(lists.all(std::slice::from_ref(level), own));
    }
    // A walk per container would visit about 300 * 300 / 2 elements.
    assert!(visits.get() <= 2 * levels.len(), "{} visits", visits.get());
}

#[test]
fn a_failing_leaf_fails_every_container_above_it_and_no_other() {
    let levels = chain(50, el("img", vec![]));
    let sibling = el("div", vec![el("p", vec![])]);
    let lists = InertLists::default();
    let own = |e: &Element| e.tag != "img";
    let both = [levels[0].clone(), sibling.clone()];
    assert!(!lists.all(&both, own));
    let failing = |l: &Node| !lists.all(std::slice::from_ref(l), own);
    assert!(levels.iter().all(failing));
    assert!(lists.all(std::slice::from_ref(&sibling), own));
    // Same verdicts as a fresh walk of each subtree.
    for level in &levels {
        let fresh = InertLists::default();
        assert_eq!(
            fresh.all(std::slice::from_ref(level), own),
            lists.all(std::slice::from_ref(level), own)
        );
    }
}
