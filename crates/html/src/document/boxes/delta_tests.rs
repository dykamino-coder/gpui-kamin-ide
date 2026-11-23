//! Публикация сообщает только повреждённые коробки и прежние поколения снятых.

use super::*;
use crate::document::delta::{BoxDelta, LayoutDamage};

fn element(id: u64, children: Vec<Node>) -> Node {
    let mut e = crate::layout::table::anon::anon_element("div", children);
    e.node_id = id;
    Node::Element(e)
}

fn publish(tree: &mut BoxTree, nodes: &[Node], previous: Option<&[Node]>) -> BoxDelta {
    tree.publish_keyed(
        nodes,
        previous,
        &publish::Keys {
            text: &[],
            element: &|id| Some(crate::dom::NodeKey::from_packed(id)),
        },
    )
}

#[test]
fn removing_a_branch_reports_every_retired_generation_and_keeps_its_neighbor() {
    let side = element(4, vec![element(5, vec![])]);
    let before = vec![element(
        1,
        vec![element(2, vec![element(3, vec![])]), side.clone()],
    )];
    let mut tree = BoxTree::default();
    publish(&mut tree, &before, None);
    let removed = [tree.box_of(2).unwrap(), tree.box_of(3).unwrap()];
    let retained = [tree.box_of(4).unwrap(), tree.box_of(5).unwrap()];
    let side_epochs = retained.map(|id| tree.epochs(id).unwrap());
    let after = vec![element(1, vec![side])];
    let delta = publish(&mut tree, &after, Some(&before));
    assert_eq!(delta.retired.len(), removed.len());
    for id in removed {
        assert!(delta.retired.contains(&id));
        assert!(tree.get(id).is_none());
    }
    for (id, epochs) in retained.into_iter().zip(side_epochs) {
        assert_eq!(tree.epochs(id), Some(epochs));
        assert!(!delta.changed.iter().any(|&(changed, _)| changed == id));
    }
    let inserted = vec![element(1, vec![element(6, vec![element(7, vec![])])])];
    publish(&mut tree, &inserted, Some(&after));
    for id in removed {
        assert!(
            tree.get(id).is_none(),
            "a reused slot cannot revive a retired key"
        );
    }
}

#[test]
fn a_paint_edit_keeps_layout_and_the_shared_sibling() {
    let before = vec![element(1, vec![element(2, vec![]), element(3, vec![])])];
    let mut tree = BoxTree::default();
    publish(&mut tree, &before, None);
    let edited = tree.box_of(2).unwrap();
    let sibling = tree.box_of(3).unwrap();
    let old = tree.epochs(edited).unwrap();
    let mut after = before.clone();
    let Node::Element(root) = &mut after[0] else {
        unreachable!()
    };
    let Node::Element(child) = &mut root.children[0] else {
        unreachable!()
    };
    child.style.cascaded_mut().inh.ui.color = Some(crate::style::values::value::Color {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    });
    let delta = publish(&mut tree, &after, Some(&before));
    assert!(delta.retired.is_empty());
    let damages: Vec<_> = delta
        .changed
        .iter()
        .filter(|&&(id, _)| id == edited)
        .collect();
    assert!(!damages.is_empty());
    assert!(damages.iter().all(|&&(_, damage)| damage.paint
        && !damage.structure
        && damage.layout == LayoutDamage::None));
    assert!(!delta.changed.iter().any(|&(id, _)| id == sibling));
    let now = tree.epochs(edited).unwrap();
    assert_eq!(
        (old.layout, old.child, old.intrinsic),
        (now.layout, now.child, now.intrinsic)
    );
    assert!(now.paint > old.paint);
}
