//! Абсолютные границы глубокого дерева не зависят от размера native-стека.

use super::{LayoutId, TaffyLayoutEngine};
use crate::{Pixels, point, size};
use taffy::prelude::TaffyMaxContent;

fn leaf(engine: &mut TaffyLayoutEngine) -> LayoutId {
    let id = engine
        .taffy
        .new_leaf(taffy::Style {
            size: taffy::Size {
                width: taffy::Dimension::length(10.5),
                height: taffy::Dimension::length(10.5),
            },
            ..taffy::Style::default()
        })
        .unwrap();
    // Каждый лист вычисляется до связывания дерева: здесь проверяется только обход границ.
    engine
        .taffy
        .compute_layout(id, taffy::Size::MAX_CONTENT)
        .unwrap();
    id.into()
}

#[test]
fn uncached_twenty_thousand_ancestors_on_small_stack() {
    std::thread::Builder::new()
        .stack_size(128 * 1024)
        .spawn(|| {
            let mut engine = TaffyLayoutEngine::new();
            let leaf = leaf(&mut engine);
            let mut root = leaf;
            for _ in 0..20_000 {
                let parent = self::leaf(&mut engine);
                engine.taffy.add_child(parent.into(), root.into()).unwrap();
                root = parent;
            }
            engine.root_origins.insert(root, (1.25, -2.25));
            let bounds = engine.layout_bounds(leaf, 1.25);
            assert_eq!(engine.absolute_unrounded[&leaf], (1.25, -2.25));
            assert_eq!(
                bounds.origin,
                point(Pixels(1.0 / 1.25), Pixels(-2.0 / 1.25))
            );
            assert_eq!(bounds.size, size(Pixels(11.0 / 1.25), Pixels(10.0 / 1.25)));
            assert_eq!(engine.absolute_layout_bounds.len(), 20_001);
            assert_eq!(engine.layout_bounds(leaf, 1.25), bounds);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn placed_origin_preserves_ancestor_caches() {
    let mut engine = TaffyLayoutEngine::new();
    let leaf = leaf(&mut engine);
    let placed = self::leaf(&mut engine);
    let root = self::leaf(&mut engine);
    engine.taffy.add_child(placed.into(), leaf.into()).unwrap();
    engine.taffy.add_child(root.into(), placed.into()).unwrap();
    engine.root_origins.insert(root, (10.25, -3.25));
    engine.placed_origins.insert(placed, (31.25, -18.25));
    let bounds = engine.layout_bounds(leaf, 1.0);
    assert_eq!(engine.absolute_unrounded[&root], (10.25, -3.25));
    assert_eq!(engine.absolute_unrounded[&placed], (31.25, -18.25));
    assert_eq!(engine.absolute_unrounded[&leaf], (31.25, -18.25));
    assert_eq!(bounds.origin, point(Pixels(31.0), Pixels(-18.0)));
    assert_eq!(bounds.size, size(Pixels(11.0), Pixels(10.0)));
    assert_eq!(engine.absolute_layout_bounds.len(), 3);
}
