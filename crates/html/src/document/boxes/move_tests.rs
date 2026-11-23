//! Stable box identity across publication order and removed ancestors.

use super::*;
use crate::document::delta::BoxDelta;
use std::collections::HashSet;

fn element(id: u64, children: Vec<Node>) -> Node {
    let mut e = crate::layout::table::anon::anon_element("div", children);
    e.node_id = id;
    Node::Element(e)
}

fn publish(tree: &mut BoxTree, nodes: &[Node], old: Option<&[Node]>) -> BoxDelta {
    tree.publish_keyed(
        nodes,
        old,
        &publish::Keys {
            text: &[],
            element: &|id| Some(crate::dom::NodeKey::from_packed(id)),
        },
    )
}

fn children(tree: &BoxTree, parent: u64) -> Vec<u64> {
    let mut at = tree.get(tree.box_of(parent).unwrap()).unwrap().first;
    let mut seen = HashSet::new();
    let mut result = vec![];
    while at.is_some() {
        assert!(seen.insert(at), "cycle in sibling links");
        let record = tree.get(at).expect("live child");
        assert_eq!(record.parent, tree.box_of(parent).unwrap());
        result.push(record.source);
        at = record.next;
    }
    result
}

#[test]
fn a_branch_move_preserves_slots_in_both_parent_orders() {
    for reverse in [false, true] {
        let branch = element(4, vec![element(5, vec![])]);
        let side = element(8, vec![element(9, vec![])]);
        let mut parents = vec![
            element(2, vec![branch.clone()]),
            element(3, vec![]),
            side.clone(),
        ];
        if reverse {
            parents.reverse();
        }
        let before = vec![element(1, parents)];
        let mut tree = BoxTree::default();
        publish(&mut tree, &before, None);
        let ids = [4, 5, 8, 9].map(|key| tree.box_of(key).unwrap());
        let epochs = [ids[2], ids[3]].map(|id| tree.epochs(id).unwrap());
        let mut parents = vec![element(2, vec![]), element(3, vec![branch]), side];
        if reverse {
            parents.reverse();
        }
        let after = vec![element(1, parents)];
        let delta = publish(&mut tree, &after, Some(&before));
        assert!(delta.retired.is_empty());
        assert_eq!([4, 5, 8, 9].map(|key| tree.box_of(key).unwrap()), ids);
        assert!(children(&tree, 2).is_empty());
        assert_eq!(children(&tree, 3), [4]);
        assert_eq!(children(&tree, 4), [5]);
        for (id, old) in [ids[2], ids[3]].into_iter().zip(epochs) {
            assert_eq!(tree.epochs(id), Some(old));
        }
        assert!(
            delta
                .changed
                .iter()
                .any(|&(id, damage)| id == ids[0] && damage.structure)
        );
    }
}

#[test]
fn moving_children_out_of_a_retired_parent_keeps_their_old_chain() {
    let before = vec![element(
        1,
        vec![
            element(
                2,
                vec![
                    element(4, vec![element(7, vec![])]),
                    element(5, vec![]),
                    element(6, vec![]),
                ],
            ),
            element(3, vec![]),
        ],
    )];
    let mut tree = BoxTree::default();
    publish(&mut tree, &before, None);
    let ids = [2, 4, 5, 6, 7].map(|key| tree.box_of(key).unwrap());
    let after = vec![element(
        1,
        vec![element(
            3,
            vec![element(5, vec![]), element(4, vec![element(7, vec![])])],
        )],
    )];
    let delta = publish(&mut tree, &after, Some(&before));
    assert_eq!(
        [4, 5, 7].map(|key| tree.box_of(key).unwrap()),
        [ids[1], ids[2], ids[4]]
    );
    assert_eq!(children(&tree, 3), [5, 4]);
    assert_eq!(children(&tree, 4), [7]);
    assert_eq!(
        delta.retired.iter().copied().collect::<HashSet<_>>(),
        HashSet::from([ids[0], ids[3]])
    );
    assert_eq!(tree.iter().count(), 5);
}

#[test]
fn simultaneous_moves_do_not_reclaim_an_already_published_box() {
    let before = vec![element(
        1,
        vec![
            element(2, vec![element(4, vec![]), element(5, vec![])]),
            element(3, vec![element(6, vec![]), element(7, vec![])]),
        ],
    )];
    let mut tree = BoxTree::default();
    publish(&mut tree, &before, None);
    let ids = [4, 5, 6, 7].map(|key| tree.box_of(key).unwrap());
    let after = vec![element(
        1,
        vec![
            element(
                2,
                vec![element(6, vec![]), element(5, vec![]), element(7, vec![])],
            ),
            element(3, vec![element(4, vec![])]),
        ],
    )];
    let delta = publish(&mut tree, &after, Some(&before));
    assert!(delta.retired.is_empty());
    assert_eq!([4, 5, 6, 7].map(|key| tree.box_of(key).unwrap()), ids);
    assert_eq!(children(&tree, 2), [6, 5, 7]);
    assert_eq!(children(&tree, 3), [4]);
    assert_eq!(tree.iter().count(), 7);
}

#[test]
fn adding_a_fragment_cannot_take_one_from_a_skipped_subtree() {
    let side = element(2, vec![element(4, vec![])]);
    let before = vec![element(1, vec![side.clone(), element(3, vec![])])];
    let mut tree = BoxTree::default();
    publish(&mut tree, &before, None);
    let original = tree.box_of(4).unwrap();
    let after = vec![element(1, vec![side, element(3, vec![element(4, vec![])])])];
    let delta = publish(&mut tree, &after, Some(&before));
    assert!(delta.retired.is_empty());
    let fragments: Vec<_> = tree.by_node.all(4).collect();
    assert_eq!(fragments.len(), 2);
    assert_eq!(fragments[0], original);
    assert_eq!(children(&tree, 2), [4]);
    assert_eq!(children(&tree, 3), [4]);
    assert_ne!(tree.get(tree.box_of(3).unwrap()).unwrap().first, original);
    let last = vec![element(1, vec![element(3, vec![element(4, vec![])])])];
    publish(&mut tree, &last, Some(&after));
    assert!(tree.get(original).is_none());
    assert_eq!(tree.box_of(4), Some(fragments[1]));
    assert_eq!(tree.by_node.all(4).collect::<Vec<_>>(), [fragments[1]]);
}

#[test]
fn a_new_fragment_before_a_retained_subtree_does_not_alias_it() {
    let side = element(2, vec![element(4, vec![])]);
    let before = vec![element(1, vec![element(3, vec![]), side.clone()])];
    let mut tree = BoxTree::default();
    publish(&mut tree, &before, None);
    let original = tree.box_of(4).unwrap();
    let after = vec![element(1, vec![element(3, vec![element(4, vec![])]), side])];
    let delta = publish(&mut tree, &after, Some(&before));
    assert!(delta.retired.is_empty());
    let fragments: Vec<_> = tree.by_node.all(4).collect();
    assert_eq!(fragments.len(), 2);
    assert_eq!(tree.get(tree.box_of(2).unwrap()).unwrap().first, original);
    assert_ne!(tree.get(tree.box_of(3).unwrap()).unwrap().first, original);
    assert_eq!(children(&tree, 2), [4]);
    assert_eq!(children(&tree, 3), [4]);
}
