//! Fragment reference boxes retain CSS phase and distinguish absent border boxes.

use super::*;

fn bounds(y: f32, height: f32) -> Bounds<Pixels> {
    Bounds {
        origin: point(px(8.0), px(y)),
        size: size(px(50.0), px(height)),
    }
}

fn scope(y: f32, height: f32) -> Scope {
    Scope {
        parent: ContentMask {
            bounds: bounds(0.0, 800.0),
        },
        fragment: bounds(y, height),
        clip: bounds(y, height),
    }
}

#[test]
fn fragment_clip_keeps_its_phase_before_scaling_and_defers_viewport_clipping() {
    let scope = scope(-20.28125, 100.0);
    let current = super::super::fragment_mask::snap(scope.clip, 1.25)
        .bounds
        .intersect(&scope.parent.bounds);
    assert_eq!(source_clip(scope, current, 1.25), scope.clip);
    let inner = bounds(10.0, 20.0);
    assert_eq!(source_clip(scope, inner, 1.25), inner);
}

#[test]
fn continued_box_uses_the_remaining_border_box_not_the_continuous_height() {
    with(Some(scope(49.71875, 100.0)), || {
        let reference = reference_box(bounds(-50.28125, 120.0)).unwrap();
        assert_eq!(reference.origin.y, px(49.71875));
        assert_eq!(reference.size.height, px(20.0));
    });
}

#[test]
fn overflow_only_reference_box_is_empty_at_fragmentainer_start() {
    with(Some(scope(49.71875, 100.0)), || {
        for y in [-150.28125, 149.71875] {
            let reference = reference_box(bounds(y, 77.0)).unwrap();
            assert_eq!(reference.origin.y, px(49.71875));
            assert_eq!(reference.size.height, px(0.0));
        }
    });
}

#[test]
fn nested_scope_restores_the_outer_fragment_reference() {
    with(Some(scope(50.0, 100.0)), || {
        with(Some(scope(100.0, 50.0)), || {
            assert_eq!(
                reference_box(bounds(0.0, 200.0)).unwrap(),
                bounds(100.0, 50.0)
            );
        });
        assert_eq!(
            reference_box(bounds(0.0, 200.0)).unwrap(),
            bounds(50.0, 100.0)
        );
    });
    assert!(reference_box(bounds(0.0, 200.0)).is_none());
}
