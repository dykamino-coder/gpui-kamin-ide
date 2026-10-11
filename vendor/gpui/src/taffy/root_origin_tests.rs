//! Separate fragment roots must share the edges of a continuous layout.

use super::{LayoutId, TaffyLayoutEngine};
use crate::{Pixels, point};
use taffy::{
    geometry::Size,
    style::{AvailableSpace, Dimension, FlexDirection, Style},
};

fn leaf(engine: &mut TaffyLayoutEngine, width: f32, height: f32) -> LayoutId {
    engine
        .taffy
        .new_leaf(Style {
            size: Size {
                width: Dimension::length(width),
                height: Dimension::length(height),
            },
            flex_shrink: 0.0,
            ..Default::default()
        })
        .unwrap()
        .into()
}

fn compute(engine: &mut TaffyLayoutEngine, root: LayoutId) {
    engine
        .taffy
        .compute_layout(
            root.into(),
            Size {
                width: AvailableSpace::MaxContent,
                height: AvailableSpace::MaxContent,
            },
        )
        .unwrap();
}

#[test]
fn intrinsic_contributions_are_independent_of_pixel_rounding() {
    for scale in [1.0, 1.25, 1.5, 2.0] {
        let mut engine = TaffyLayoutEngine::new();
        let child = leaf(&mut engine, 25.0 * scale, 100.0 * scale);
        compute(&mut engine, child);
        for x in [0.0, 0.4, 8.1] {
            engine.set_root_origin(child, point(Pixels(x), Pixels(0.0)), scale);
            let raw = engine.layout_size_unrounded(child, scale);
            assert_eq!(raw.width, Pixels(25.0));
            assert_eq!(raw.width.0 * 4.0, 100.0);
            let painted = engine.layout_bounds(child, scale);
            let device_width = painted.size.width.0 * scale;
            assert!((device_width - device_width.round()).abs() < 0.0001);
            assert_eq!(
                device_width,
                ((x + 25.0) * scale).round() - (x * scale).round()
            );
        }
    }
}

#[test]
fn separate_roots_match_continuous_layout_on_fractional_displays() {
    for scale in [1.0, 1.25, 1.5, 2.0] {
        let mut engine = TaffyLayoutEngine::new();
        let children: Vec<LayoutId> = (0..4)
            .map(|_| leaf(&mut engine, 25.0 * scale, 100.0 * scale))
            .collect();
        let parent: LayoutId = engine
            .taffy
            .new_with_children(
                Style {
                    flex_direction: FlexDirection::Row,
                    ..Default::default()
                },
                &children.iter().copied().map(Into::into).collect::<Vec<_>>(),
            )
            .unwrap()
            .into();
        compute(&mut engine, parent);
        engine.set_root_origin(parent, point(Pixels(8.0), Pixels(17.3)), scale);
        for (column, child) in children.into_iter().enumerate() {
            let expected = engine.layout_bounds(child, scale);
            let fragment = leaf(&mut engine, 25.0 * scale, 100.0 * scale);
            compute(&mut engine, fragment);
            // Populate a zero-origin cache, as intrinsic measurement does.
            engine.layout_bounds(fragment, scale);
            engine.set_root_origin(
                fragment,
                point(Pixels(8.0 + column as f32 * 25.0), Pixels(17.3)),
                scale,
            );
            assert_eq!(engine.layout_bounds(fragment, scale), expected);
        }
    }
}

#[test]
fn moving_a_root_invalidates_cached_descendant_bounds() {
    let mut engine = TaffyLayoutEngine::new();
    let child = leaf(&mut engine, 6.25, 12.5);
    let parent: LayoutId = engine
        .taffy
        .new_with_children(Style::default(), &[child.into()])
        .unwrap()
        .into();
    compute(&mut engine, parent);
    let initial = engine.layout_bounds(child, 1.25);
    engine.set_root_origin(parent, point(Pixels(-0.6), Pixels(-23.2)), 1.25);
    let moved = engine.layout_bounds(child, 1.25);
    assert_ne!(moved.origin, initial.origin);
    assert_eq!(moved.origin.x, Pixels(-0.8));
    assert_eq!(moved.origin.y, Pixels(-23.2));
    // Repositioning again must not keep either prior root's cached bounds.
    engine.set_root_origin(parent, point(Pixels(8.0), Pixels(16.0)), 1.25);
    let reset = engine.layout_bounds(child, 1.25);
    assert_eq!(reset.origin, point(Pixels(8.0), Pixels(16.0)));
    assert_eq!(reset.size, initial.size);
}

#[test]
fn positioned_subtree_rounds_after_translation_without_changing_layout() {
    for scale in [1.0, 1.25, 1.5, 2.0] {
        let mut engine = TaffyLayoutEngine::new();
        let grandchild = leaf(&mut engine, 50.0 * scale, 70.0 * scale);
        let moved: LayoutId = engine
            .taffy
            .new_with_children(Style::default(), &[grandchild.into()])
            .unwrap()
            .into();
        let sibling = leaf(&mut engine, 50.0 * scale, 30.0 * scale);
        let parent: LayoutId = engine
            .taffy
            .new_with_children(Style::default(), &[moved.into(), sibling.into()])
            .unwrap()
            .into();
        compute(&mut engine, parent);
        engine.set_root_origin(parent, point(Pixels(8.0), Pixels(8.0)), scale);
        let original_layout = engine.layout_exact(moved, scale);
        let sibling_bounds = engine.layout_bounds(sibling, scale);
        engine.layout_bounds(grandchild, scale);

        for target in [
            point(Pixels(58.0), Pixels(38.0)),
            point(Pixels(-0.3), Pixels(0.3)),
        ] {
            engine.set_placed_origin(moved, target, scale);
            let bounds = engine.layout_bounds(grandchild, scale);
            let x = target.x.0 * scale;
            let y = target.y.0 * scale;
            assert_eq!(
                bounds.origin,
                point(Pixels(x.round() / scale), Pixels(y.round() / scale))
            );
            assert_eq!(
                bounds.size.width,
                Pixels(((x + 50.0 * scale).round() - x.round()) / scale)
            );
            assert_eq!(
                bounds.size.height,
                Pixels(((y + 70.0 * scale).round() - y.round()) / scale)
            );
            assert_eq!(engine.layout_exact(moved, scale), original_layout);
            assert_eq!(engine.layout_bounds(sibling, scale), sibling_bounds);
        }
    }
}
