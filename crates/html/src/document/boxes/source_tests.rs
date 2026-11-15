//! Синтетическая коробка определяется владельцем и источником, а не номером обхода.

use super::*;
use crate::dom::{Element, NodeKey, Text};

fn element(key: u32, children: Vec<Node>) -> Element {
    let mut e = crate::layout::table::anon::anon_element("div", children);
    e.node_id = NodeKey {
        slot: key,
        generation: 1,
    }
    .packed();
    e
}

fn wrapper(child: Node) -> Node {
    let mut e = crate::layout::table::anon::anon_element("table", vec![child]);
    e.marks.anonymous_wrapper = true;
    // Старый путь таблиц выдаёт производный node_id: это не ключ DOM.
    e.node_id = 123;
    Node::Element(e)
}

#[test]
fn hiding_an_anonymous_sibling_does_not_transfer_its_box_id() {
    let left = wrapper(Node::Element(element(2, vec![])));
    let right = wrapper(Node::Element(element(3, vec![])));
    let mut tree = BoxTree::default();
    tree.publish(&[Node::Element(element(1, vec![left, right.clone()]))]);
    let parent = tree.first;
    let removed = tree.get(parent).unwrap().first;
    let retained = tree.get(removed).unwrap().next;
    tree.publish(&[Node::Element(element(1, vec![right]))]);
    assert_eq!(tree.first, parent);
    assert_eq!(tree.get(parent).unwrap().first, retained);
    assert!(tree.get(removed).is_none());
}

#[test]
fn text_fragment_identity_uses_dom_keys_instead_of_text_order() {
    let left_key = NodeKey {
        slot: 7,
        generation: 1,
    };
    let right_key = NodeKey {
        slot: 9,
        generation: 1,
    };
    let text =
        |value: &str, id| Node::Text(Text::from_source(value.into(), crate::dom::TextId::new(id)));
    let left = wrapper(text("left", 1));
    let right = wrapper(text("right", 2));
    let mut tree = BoxTree::default();
    tree.publish_sources(
        &[Node::Element(element(1, vec![left, right]))],
        &[left_key, right_key],
    );
    let parent = tree.first;
    let removed = tree.get(parent).unwrap().first;
    let retained = tree.get(removed).unwrap().next;
    let right = wrapper(text("edited", 1));
    tree.publish_sources(&[Node::Element(element(1, vec![right]))], &[right_key]);
    assert_eq!(tree.get(parent).unwrap().first, retained);
    assert!(tree.get(removed).is_none());
}

#[test]
fn moving_an_anonymous_fragment_to_another_owner_creates_a_new_box() {
    let fragment = wrapper(Node::Element(element(3, vec![])));
    let mut tree = BoxTree::default();
    tree.publish(&[Node::Element(element(1, vec![fragment.clone()]))]);
    let old = tree.get(tree.first).unwrap().first;
    tree.publish(&[Node::Element(element(2, vec![fragment]))]);
    let new = tree.get(tree.first).unwrap().first;
    assert_ne!(old, new);
    assert!(tree.get(old).is_none());
}

#[test]
fn a_generated_marker_does_not_share_its_authored_sources_box() {
    let authored = element(3, vec![]);
    let mut marker = authored.clone();
    marker.marks.box_origin = Some(crate::style::box_input::BoxOrigin::Marker);
    let mut tree = BoxTree::default();
    tree.publish(&[Node::Element(element(
        1,
        vec![Node::Element(authored), Node::Element(marker.clone())],
    ))]);
    let parent = tree.first;
    let removed = tree.get(parent).unwrap().first;
    let retained = tree.get(removed).unwrap().next;
    tree.publish(&[Node::Element(element(1, vec![Node::Element(marker)]))]);
    assert_eq!(tree.get(parent).unwrap().first, retained);
    assert!(tree.get(removed).is_none());
}

#[test]
fn the_synthetic_viewport_survives_a_change_of_child_extent() {
    let viewport = |children| {
        Node::Element(crate::layout::table::anon::anon_element(
            "html-viewport",
            children,
        ))
    };
    let right = Node::Element(element(3, vec![]));
    let mut tree = BoxTree::default();
    tree.publish(&[viewport(vec![
        Node::Element(element(2, vec![])),
        right.clone(),
    ])]);
    let root = tree.first;
    let removed = tree.get(root).unwrap().first;
    let retained = tree.get(removed).unwrap().next;
    tree.publish(&[viewport(vec![right])]);
    assert_eq!(tree.first, root);
    assert_eq!(tree.get(root).unwrap().first, retained);
    assert!(tree.get(removed).is_none());
}
