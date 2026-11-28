//! KaminIDE patch: only the window's frame tree grows to the frame's element count,
//! and only once it is large; small trees keep doubling.

use super::{RESERVE_FROM_NODES, TaffyLayoutEngine};
use std::cell::Cell;
use taffy::style::Style;

/// Insert nodes until the storage is full with at least `nodes` nodes.
fn fill(engine: &mut TaffyLayoutEngine, nodes: usize) {
    while !engine.taffy.node_storage_full() || engine.taffy.total_node_count() < nodes {
        engine.taffy.new_leaf(Style::default()).unwrap();
    }
}

#[test]
fn a_full_large_frame_tree_grows_once_to_the_hint() {
    let mut engine = TaffyLayoutEngine::new();
    fill(&mut engine, RESERVE_FROM_NODES);
    let asked = Cell::new(0);
    engine.reserve_nodes_when_full(|| {
        asked.set(asked.get() + 1);
        RESERVE_FROM_NODES * 3
    });
    assert_eq!(asked.get(), 1);
    assert!(!engine.taffy.node_storage_full());
    // Storage with room left does not read the hint again.
    engine.reserve_nodes_when_full(|| unreachable!("storage is not full"));
}

#[test]
fn a_small_full_frame_tree_keeps_doubling() {
    let mut engine = TaffyLayoutEngine::new();
    fill(&mut engine, 1);
    assert!(engine.taffy.total_node_count() < RESERVE_FROM_NODES);
    engine.reserve_nodes_when_full(|| unreachable!("small trees do not read the hint"));
    assert!(engine.taffy.node_storage_full());
}

#[test]
fn an_independent_subtree_keeps_only_its_own_nodes() {
    let mut engine = TaffyLayoutEngine::new_for_subtree();
    fill(&mut engine, RESERVE_FROM_NODES);
    engine.reserve_nodes_when_full(|| unreachable!("subtrees are not sized by the frame"));
    assert!(engine.taffy.node_storage_full());
}
