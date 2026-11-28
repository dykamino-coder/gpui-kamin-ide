//! Storage follows current geometry and retained nodes without invalidating either.

use super::{leaf, measured};
use crate::{Bounds, Pixels, point, size, taffy::TaffyLayoutEngine};
use std::rc::Rc;

fn fill_geometry(engine: &mut TaffyLayoutEngine, count: usize) -> [usize; 5] {
    for i in 0..count {
        let id = leaf(engine, i as f32);
        engine.absolute_unrounded.insert(id, (i as f32, 1.));
        engine.absolute_layout_bounds.insert(
            id,
            Bounds::new(
                point(Pixels(i as f32), Pixels(1.)),
                size(Pixels(2.), Pixels(3.)),
            ),
        );
        engine.root_origins.insert(id, (i as f32, 2.));
        engine.placed_origins.insert(id, (i as f32, 3.));
        engine.computed_layouts.insert(id);
    }
    capacities(engine)
}

fn capacities(engine: &TaffyLayoutEngine) -> [usize; 5] {
    [
        engine.absolute_unrounded.capacity(),
        engine.absolute_layout_bounds.capacity(),
        engine.root_origins.capacity(),
        engine.placed_origins.capacity(),
        engine.computed_layouts.capacity(),
    ]
}

#[test]
fn geometry_tables_keep_stable_capacity_and_release_a_previous_large_pass() {
    let mut engine = TaffyLayoutEngine::new();
    let large = fill_geometry(&mut engine, 4096);
    engine.clear();
    assert_eq!(capacities(&engine), large);
    fill_geometry(&mut engine, 2);
    engine.clear();
    let small = capacities(&engine);
    assert!(
        small
            .into_iter()
            .zip(large)
            .all(|(small, large)| small >= 2 && small < large)
    );
    for _ in 0..3 {
        fill_geometry(&mut engine, 2);
        engine.clear();
        assert_eq!(capacities(&engine), small);
    }
    assert!(engine.absolute_layout_bounds.is_empty());
    assert!(engine.absolute_unrounded.is_empty());
    assert!(engine.root_origins.is_empty());
    assert!(engine.placed_origins.is_empty());
    assert!(engine.computed_layouts.is_empty());
    // A pass with no geometry must release all former geometry buckets too.
    engine.clear();
    assert_eq!(capacities(&engine), [0; 5]);
}

fn retained_bytes(engine: &TaffyLayoutEngine) -> usize {
    let mut bytes = 0;
    engine.heap_report(&mut |owner, _, size| {
        if owner == "gpui/retained-layout" {
            bytes += size;
        }
    });
    bytes
}

#[test]
fn a_small_retained_tree_releases_history_but_keeps_its_stamp_and_memo() {
    let mut engine = TaffyLayoutEngine::new();
    engine.set_layout_keys(true);
    for key in 0..4096 {
        let id = measured(&mut engine, Some(key));
        engine.set_layout_memo(id, Rc::new(key));
    }
    engine.clear();
    let large = retained_bytes(&engine);
    assert!(large > 4096 * std::mem::size_of::<u64>());
    let memo: Rc<dyn std::any::Any> = Rc::new(123_u64);
    let mut kept = None;
    let mut stamp = None;
    let mut settled_bytes = None;
    for pass in 0..4 {
        engine.set_layout_keys(true);
        let id = measured(&mut engine, Some(7));
        if let Some(kept) = kept {
            assert_eq!(id, kept);
            assert_eq!(engine.layout_stamp(id), stamp);
            assert!(Rc::ptr_eq(
                &engine.layout_memo(id).expect("kept memo"),
                &memo
            ));
        } else {
            kept = Some(id);
            stamp = engine.layout_stamp(id);
            engine.set_layout_memo(id, memo.clone());
        }
        engine.clear();
        assert_eq!(engine.taffy.total_node_count(), 1);
        // The first small request reserved its sequence from the large pass;
        // the following one reserves from the actual single-node sequence.
        if pass >= 1 {
            let small = retained_bytes(&engine);
            assert!(small < large / 16);
            if let Some(previous) = settled_bytes {
                assert_eq!(small, previous);
            }
            settled_bytes = Some(small);
        }
    }
    // Empty passes retire the memo and all candidate tables as well.
    engine.clear();
    engine.clear();
    assert_eq!(retained_bytes(&engine), 0);
    assert_eq!(Rc::strong_count(&memo), 1);
}
