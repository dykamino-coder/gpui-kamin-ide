//! Проверки семантики доказательств и линейного переиспользования снимков.

use super::*;
use crate::layout::table::anon::anon_element;

#[test]
fn inline_text_does_not_force_margin_rewrites_at_every_ancestor() {
    let leaf = anon_element("span", vec![Node::Text("leaf".into())]);
    let mut nodes = vec![Node::Element(leaf)];
    for _ in 0..1000 {
        nodes = vec![Node::Element(anon_element("div", nodes))];
    }
    let mut facts = SubtreeFacts::default();
    assert!(facts.margins_unchanged(&nodes));
    let mut next = nodes.as_slice();
    let mut entries = 0;
    while let Some(Node::Element(e)) = next.first() {
        assert!(e.children.box_summary().is_some());
        assert!(facts.margins_unchanged(next));
        next = &e.children;
        entries += 1;
    }
    assert_eq!(entries, 1001);
}

#[test]
fn block_inside_inline_still_requires_preparation() {
    let block = anon_element("div", vec![Node::Text("block".into())]);
    let inline = anon_element("span", vec![Node::Element(block)]);
    let nodes = vec![Node::Element(inline)];
    assert!(!SubtreeFacts::default().margins_unchanged(&nodes));
}

#[test]
fn prepared_text_facts_exclude_hidden_floated_and_positioned_descendants() {
    for (display, float, position, visible) in [
        (None, None, None, true),
        (Some(Display::Contents), None, None, true),
        (Some(Display::None), None, None, false),
        (None, Some(1), None, false),
        (None, Some(2), None, false),
        (
            None,
            None,
            Some(crate::style::computed::Position::Absolute),
            false,
        ),
        (
            None,
            None,
            Some(crate::style::computed::Position::Fixed),
            false,
        ),
    ] {
        let mut child = anon_element("span", vec![Node::Text("text".into())]);
        let style = child.style.used_mut();
        style.reset_box_layout.display = display;
        style.reset_box_layout.float = float;
        style.reset_positioned.position = position;
        let root = anon_element("span", vec![Node::Element(child)]);
        let mut facts = SubtreeFacts::default();
        facts.breaks_absent(&root);
        let text = root.children.box_summary().unwrap().text;
        assert_eq!(text.in_flow, visible);
        assert!(text.elements);
        assert!(!text.nonempty);
        assert!(!text.significant);
    }
}

#[test]
fn cow_style_change_invalidates_shared_proofs() {
    let mut root = anon_element("div", vec![Node::Text("leaf".into())]);
    for _ in 0..1000 {
        root = anon_element("div", vec![Node::Element(root)]);
    }
    let mut facts = SubtreeFacts::default();
    assert!(facts.breaks_absent(&root));
    assert!(facts.floats_absent(&root));
    assert!(facts.margins_unchanged(&root.children));
    let old = root.clone();
    let Node::Element(child) = &mut root.children[0] else {
        unreachable!()
    };
    child.style.used_mut().reset_fragmentation.break_after_avoid = true;
    child.style.used_mut().reset_box_layout.float = Some(1);
    child.style.used_mut().reset_box_layout.margin.top = Some(Len::Px(10.));
    assert!(!facts.breaks_absent(&root));
    assert!(!facts.floats_absent(&root));
    assert!(!facts.margins_unchanged(&root.children));
    assert!(facts.breaks_absent(&old));
    assert!(facts.floats_absent(&old));
    assert!(facts.margins_unchanged(&old.children));
}

#[test]
fn clearance_keeps_hover_and_first_letter_float_possibilities() {
    let mut element = anon_element("div", vec![]);
    element.style.used_mut().reset_box_layout.clear = Some(0);
    let mut facts = SubtreeFacts::default();
    assert!(!facts.clearance_possible(&[Node::Element(element.clone())]));
    let mut floated = Computed::default();
    floated.reset_box_layout.float = Some(1);
    element.hover = Some(floated.clone().into());
    assert!(facts.clearance_possible(&[Node::Element(element.clone())]));
    element.hover = None;
    element.first_letter = Some(floated.into());
    assert!(facts.clearance_possible(&[Node::Element(element)]));
}

#[test]
fn break_proof_is_shared_and_cow_style_change_invalidates_it() {
    let mut root = anon_element("div", vec![Node::Text("leaf".into())]);
    for _ in 0..2052 {
        root = anon_element("div", vec![Node::Element(root)]);
    }
    let mut facts = SubtreeFacts::default();
    assert!(facts.breaks_absent(&root));
    assert!(facts.floats_absent(&root));
    let proof = root.children.box_summary();
    assert!(facts.breaks_absent(&root));
    assert_eq!(root.children.box_summary(), proof);
    let old = root.clone();
    let Node::Element(child) = &mut root.children[0] else {
        unreachable!()
    };
    child.style.used_mut().reset_fragmentation.break_after_avoid = true;
    child.style.used_mut().reset_box_layout.float = Some(1);
    assert!(!facts.breaks_absent(&root));
    assert!(!facts.floats_absent(&root));
    assert!(facts.breaks_absent(&old));
    assert!(facts.floats_absent(&old));
}

#[test]
fn cow_margin_change_invalidates_the_subtree_proof() {
    let mut nodes = vec![Node::Text("leaf".into())];
    for _ in 0..2052 {
        nodes = vec![Node::Element(crate::layout::table::anon::anon_element(
            "div", nodes,
        ))];
    }
    let mut facts = SubtreeFacts::default();
    assert!(facts.margins_unchanged(&nodes));
    let old = nodes.clone();
    let Node::Element(root) = &mut nodes[0] else {
        unreachable!()
    };
    let Node::Element(child) = &mut root.children[0] else {
        unreachable!()
    };
    child.style.used_mut().reset_box_layout.margin.top = Some(Len::Px(10.));
    assert!(!facts.margins_unchanged(&nodes));
    assert!(facts.margins_unchanged(&old));
}

#[test]
fn nonzero_margins_do_not_require_structural_inline_preparation() {
    let mut leaf = anon_element("span", vec![Node::Text("leaf".into())]);
    leaf.style.used_mut().reset_box_layout.margin.top = Some(Len::Px(12.));
    let mut nodes = vec![Node::Element(leaf)];
    for _ in 0..1000 {
        nodes = vec![Node::Element(anon_element("div", nodes))];
    }
    let mut facts = SubtreeFacts::default();
    assert!(!facts.margins_unchanged(&nodes));
    let mut next = nodes.as_slice();
    let mut entries = 0;
    while let Some(Node::Element(e)) = next.first() {
        assert!(facts.inline_preparation_absent(next));
        next = &e.children;
        entries += 1;
    }
    assert_eq!(entries, 1001);
    let block = anon_element("div", vec![]);
    let inline = anon_element("span", vec![Node::Element(block)]);
    assert!(!facts.inline_preparation_absent(&[Node::Element(inline)]));
}
