//! KaminIDE patch: only the window's frame tree grows to the frame's element count.

use super::TaffyLayoutEngine;
use std::cell::Cell;
use taffy::style::Style;

fn fill(engine: &mut TaffyLayoutEngine) {
    while !engine.taffy.node_storage_full() {
        engine.taffy.new_leaf(Style::default()).unwrap();
    }
}

#[test]
fn a_full_frame_tree_grows_once_to_the_hint() {
    let mut engine = TaffyLayoutEngine::new();
    fill(&mut engine);
    let asked = Cell::new(0);
    engine.reserve_nodes_when_full(|| {
        asked.set(asked.get() + 1);
        5000
    });
    assert_eq!(asked.get(), 1);
    assert!(!engine.taffy.node_storage_full());
    // Storage with room left does not read the hint again.
    engine.reserve_nodes_when_full(|| unreachable!("storage is not full"));
}

#[test]
fn an_independent_subtree_keeps_only_its_own_nodes() {
    let mut engine = TaffyLayoutEngine::new_for_subtree();
    assert!(engine.taffy.node_storage_full());
    engine.reserve_nodes_when_full(|| unreachable!("subtrees are not sized by the frame"));
    assert!(engine.taffy.node_storage_full());
}
