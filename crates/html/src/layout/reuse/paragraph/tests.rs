//! Tests of the atom paths and record keys of kept paragraphs (`super`).

use super::*;

fn el(node_id: u64, children: Vec<Node>) -> Node {
    let mut e = crate::layout::table::anon::anon_element("i", children);
    e.node_id = node_id;
    Node::Element(e)
}

#[test]
fn atom_paths_follow_document_order_below_the_paragraph() {
    let nodes = vec![
        el(1, vec![el(10, vec![]), el(11, vec![])]),
        el(2, vec![]),
        el(3, vec![el(30, vec![el(300, vec![])])]),
    ];
    let atoms = plan::paths(&nodes, vec![10, 11, 2, 300]).expect("all found");
    assert_eq!(atoms.keys, [10, 11, 2, 300]);
    assert_eq!(atoms.path_of(0), &[0, 0]);
    assert_eq!(atoms.path_of(1), &[0, 1]);
    assert_eq!(atoms.path_of(2), &[1]);
    assert_eq!(atoms.path_of(3), &[2, 0, 0]);
}

#[test]
fn an_atom_out_of_order_or_missing_has_no_path() {
    let nodes = vec![el(1, vec![]), el(2, vec![])];
    assert!(plan::paths(&nodes, vec![2, 1]).is_none());
    assert!(plan::paths(&nodes, vec![5]).is_none());
    assert!(plan::paths(&nodes, vec![]).is_some_and(|a| a.keys.is_empty()));
}

#[test]
fn the_record_key_follows_the_top_level_nodes() {
    let a = vec![el(1, vec![]), el(2, vec![])];
    let b = vec![el(1, vec![]), el(3, vec![])];
    assert_eq!(key::id(7, &a), key::id(7, &a));
    assert_ne!(key::id(7, &a), key::id(7, &b));
    assert_ne!(key::id(7, &a), key::id(8, &a));
}
