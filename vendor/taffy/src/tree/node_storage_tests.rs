use super::{NodeData, NodeStorage, PAGE_NODES};
use crate::{
    Style, TaffyTree,
    geometry::Size,
    prelude::{TaffyMaxContent, length},
};

#[test]
fn a_tiny_probe_reserves_only_its_requested_payloads() {
    let mut empty = NodeStorage::with_capacity(0);
    empty.insert(NodeData::new(Style::default().into()));
    assert_eq!(empty.pages.len(), 1);
    assert!(empty.pages[0].capacity() <= 2);
    assert_eq!(empty.capacity(), empty.pages[0].capacity());
    let mut hinted = NodeStorage::with_capacity(16);
    assert_eq!(hinted.pages[0].capacity(), 16);
    assert_eq!(hinted.capacity(), 16);
    for _ in 0..65 {
        hinted.insert(NodeData::new(Style::default().into()));
    }
    assert_eq!(hinted.pages[0].capacity(), PAGE_NODES);
    assert_eq!(hinted.pages.len(), 2);
    assert!(
        hinted
            .pages
            .iter()
            .all(|page| page.capacity() <= PAGE_NODES)
    );
}

#[test]
fn removal_moves_payloads_without_changing_survivor_or_recycled_key_identity() {
    let mut nodes = NodeStorage::with_capacity(0);
    let keys: Vec<_> = (0..257)
        .map(|index| {
            let mut value = NodeData::new(Style::default().into());
            value.content_shift = index as f32;
            nodes.insert(value)
        })
        .collect();
    for index in (0..keys.len()).filter(|index| index % 3 != 1) {
        assert_eq!(
            nodes.remove(keys[index]).unwrap().content_shift,
            index as f32
        );
        assert!(nodes.get(keys[index]).is_none());
        assert!(nodes.remove(keys[index]).is_none());
    }
    for index in (0..keys.len()).filter(|index| index % 3 == 1) {
        assert_eq!(nodes[keys[index]].content_shift, index as f32);
    }
    assert_eq!(
        nodes.pages.len(),
        (nodes.len() + PAGE_NODES - 1) / PAGE_NODES
    );
    let replacement = nodes.insert(NodeData::new(Style::default().into()));
    assert!(nodes.get(replacement).is_some());
    for (index, key) in keys.iter().enumerate() {
        if index % 3 == 1 {
            assert_eq!(nodes[*key].content_shift, index as f32);
        } else {
            assert_ne!(replacement, *key);
            assert!(nodes.get(*key).is_none());
        }
    }
}

#[test]
fn clear_releases_payloads_and_invalidates_keys_without_resetting_generations() {
    let mut nodes = NodeStorage::with_capacity(1000);
    let old = nodes.insert(NodeData::new(Style::default().into()));
    nodes.clear();
    assert!(nodes.pages.is_empty());
    let new = nodes.insert(NodeData::new(Style::default().into()));
    assert_ne!(old, new);
    assert!(nodes.get(old).is_none());
    assert_eq!(nodes.values().count(), 1);
}

#[cfg(feature = "std")]
#[test]
fn a_large_retired_tree_releases_payload_pages_and_keeps_live_layout_context_and_links() {
    let mut tree = TaffyTree::new();
    tree.disable_rounding();
    let child = tree
        .new_leaf_with_context(
            Style {
                size: Size {
                    width: length(23.0),
                    height: length(17.0),
                },
                ..Style::default()
            },
            42,
        )
        .unwrap();
    let root = tree.new_with_children(Style::default(), &[child]).unwrap();
    let dead: Vec<_> = (0..4096)
        .map(|_| tree.new_leaf(Style::default()).unwrap())
        .collect();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    let layout = *tree.layout(child).unwrap();
    let footprint = |tree: &TaffyTree<i32>| {
        let mut bytes = 0;
        tree.heap_report(&mut |owner, _, count| {
            if owner == "layout/taffy-nodes" {
                bytes += count;
            }
        });
        bytes
    };
    let before = footprint(&tree);
    tree.retain_nodes(|node| node == root || node == child);
    let after = footprint(&tree);
    assert!(
        after < before / 4,
        "retired payload storage: {before} -> {after}"
    );
    assert_eq!(tree.total_node_count(), 2);
    assert_eq!(tree.parent(child), Some(root));
    assert_eq!(tree.children(root).unwrap(), [child]);
    assert_eq!(tree.get_node_context(child), Some(&42));
    assert_eq!(*tree.layout(child).unwrap(), layout);
    assert!(!tree.dirty(child).unwrap());
    for node in &dead {
        assert!(!tree.nodes.contains_key((*node).into()));
    }
    tree.set_style(
        child,
        Style {
            size: Size {
                width: length(31.0),
                height: length(19.0),
            },
            ..Style::default()
        },
    )
    .unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(
        tree.layout(child).unwrap().size,
        Size {
            width: 31.0,
            height: 19.0
        }
    );
    let added = tree.new_leaf_with_context(Style::default(), 7).unwrap();
    assert!(!dead.contains(&added));
    tree.add_child(root, added).unwrap();
    assert_eq!(tree.parent(added), Some(root));
    assert_eq!(tree.get_node_context(added), Some(&7));
    assert_eq!(tree.get_node_context(child), Some(&42));
}
