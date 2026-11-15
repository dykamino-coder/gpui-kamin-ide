//! Коробка освобождается при Remove, а ключ исходного DOM живёт до Drop.

use super::*;
use html5ever::tree_builder::{NodeOrText, TreeSink};
use html5ever::{QualName, local_name, ns};
use markup5ever_rcdom::{Handle, RcDom};

fn authored(dom: &RcDom) -> Handle {
    dom.create_element(
        QualName::new(None, ns!(html), local_name!("div")),
        vec![],
        Default::default(),
    )
}

fn project(handle: &Handle) -> Node {
    let mut element = crate::layout::table::anon::anon_element("div", vec![]);
    element.node_id = handle.key().packed();
    Node::Element(element)
}

#[test]
fn detach_retires_only_its_box_and_reinsert_keeps_the_dom_key() {
    let dom = RcDom::default();
    let left = authored(&dom);
    let right = authored(&dom);
    for node in [&left, &right] {
        dom.append(&dom.document, NodeOrText::AppendNode(node.clone()));
    }
    let key = left.key();
    let mut tree = BoxTree::default();
    tree.publish(&[project(&left), project(&right)]);
    let removed = tree.first;
    let retained = tree.get(removed).unwrap().next;
    dom.remove_from_parent(&left);
    tree.publish(&[project(&right)]);
    assert!(tree.get(removed).is_none());
    assert_eq!(tree.first, retained);
    assert_eq!(left.key(), key);
    dom.append_before_sibling(&right, NodeOrText::AppendNode(left.clone()));
    tree.publish(&[project(&left), project(&right)]);
    assert_ne!(tree.first, removed);
    assert_eq!(tree.get(tree.first).unwrap().next, retained);
    assert_eq!(left.key(), key);
}

#[test]
fn a_new_dom_node_never_inherits_the_dropped_nodes_box() {
    let dom = RcDom::default();
    let old = authored(&dom);
    let key = old.key();
    let mut tree = BoxTree::default();
    tree.publish(&[project(&old)]);
    let old_box = tree.first;
    drop(old);
    let next = authored(&dom);
    assert_ne!(next.key(), key);
    tree.publish(&[project(&next)]);
    assert_ne!(tree.first, old_box);
    assert!(tree.get(old_box).is_none());
    assert_eq!(
        tree.get(tree.first).unwrap().origin,
        Source::Node(next.key())
    );
}

#[test]
fn the_text_source_index_records_keys_without_retaining_nodes() {
    let dom = RcDom::default();
    let text = dom.new_node(markup5ever_rcdom::NodeData::Text {
        contents: std::cell::RefCell::new("text".into()),
    });
    let key = text.key();
    let traversal = crate::values::traversal::TraversalState::default();
    let id = traversal.record_text(&text).unwrap();
    assert_eq!(traversal.text_keys()[id.get() as usize - 1], key);
    drop(text);
    assert_eq!(traversal.text_keys()[id.get() as usize - 1], key);
}
