//! CSS Break 3 §5.5: graphical effects use each fragment's reference box.
//!
//! The continuous copy still owns layout and containing-block geometry. Only
//! the effect reference box and the fragment's source clip change at paint.

use crate::paint::context::PaintCx;
use gpui::{Bounds, ContentMask, Pixels, TransformationMatrix, Window, point, px, size};

#[derive(Clone, Copy)]
pub(crate) struct Scope {
    pub parent: ContentMask<Pixels>,
    pub fragment: Bounds<Pixels>,
    pub clip: Bounds<Pixels>,
}

pub(crate) fn with<R>(owner: &PaintCx, scope: Option<Scope>, f: impl FnOnce() -> R) -> R {
    let previous = owner.effect.replace(scope);
    let result = f();
    owner.effect.replace(previous);
    result
}

/// Краска идёт внутри фрагмента внешней фрагментации.
pub(crate) fn active(owner: &PaintCx) -> bool {
    owner.effect.borrow().is_some()
}

pub(crate) fn reference_box(
    owner: Option<&PaintCx>,
    bounds: Bounds<Pixels>,
) -> Option<Bounds<Pixels>> {
    let scope = *owner?.effect.borrow();
    let scope = scope?;
    let start = bounds.origin.y.max(scope.fragment.origin.y);
    let end = (bounds.origin.y + bounds.size.height)
        .min(scope.fragment.origin.y + scope.fragment.size.height);
    // An overflow-only fragment has a zero-height reference box at the
    // fragmentainer start, rather than the old box's now negative origin.
    Some(Bounds {
        origin: point(
            bounds.origin.x,
            if end <= start {
                scope.fragment.origin.y
            } else {
                start
            },
        ),
        size: size(bounds.size.width, (end - start).max(px(0.0))),
    })
}

/// CSS Break 3 §5.4: slice the continuous shadow at internal fragment edges,
/// retaining its ink overflow at the box's genuine outer edges.
pub(crate) fn paint_shadow<R>(
    owner: Option<&PaintCx>,
    bounds: Bounds<Pixels>,
    ink: (Pixels, Pixels),
    window: &mut Window,
    f: impl FnOnce(&mut Window) -> R,
) -> R {
    let Some(scope) = owner.and_then(|owner| *owner.effect.borrow()) else {
        return f(window);
    };
    let current = window.content_mask().bounds;
    let initial = super::fragment_mask::snap(scope.clip, window.scale_factor())
        .bounds
        .intersect(&scope.parent.bounds);
    // A CSS ancestor's overflow clip still clips the shadow.
    if current != initial {
        return f(window);
    }
    let mut top = current.top();
    let mut bottom = current.bottom();
    if bounds.top() >= scope.fragment.top() - px(0.01) {
        top = top.min(ink.0);
    }
    if bounds.bottom() <= scope.fragment.bottom() + px(0.01) {
        bottom = bottom.max(ink.1);
    }
    let clip = Bounds {
        origin: point(current.left(), top),
        size: size(current.size.width, bottom - top),
    };
    let mask = super::fragment_mask::snap(clip, window.scale_factor());
    window.with_content_mask_replaced(
        ContentMask {
            bounds: mask.bounds.intersect(&scope.parent.bounds),
        },
        f,
    )
}

pub(crate) fn paint<R>(
    owner: Option<&PaintCx>,
    matrix: TransformationMatrix,
    clip_matrix: TransformationMatrix,
    clip_ancestor: bool,
    window: &mut Window,
    f: impl FnOnce(&mut Window) -> R,
) -> R {
    let scope = owner.and_then(|owner| *owner.effect.borrow());
    let [[a, b], [c, d]] = matrix.rotation_scale;
    let axes = (b.abs() < 1e-4 && c.abs() < 1e-4) || (a.abs() < 1e-4 && d.abs() < 1e-4);
    // A descendant transform must not move a CSS ancestor's clip. Its
    // reference box remains fragmented, but clip projection needs the
    // fragment's own mask, without additional subtree clipping.
    if let Some(scope) = scope.filter(|_| axes && !clip_ancestor) {
        let mut css = clip_matrix;
        for v in &mut css.translation {
            *v /= window.scale_factor();
        }
        let current = window.content_mask().bounds;
        let source = source_clip(scope, current, window.scale_factor());
        let p = css.apply(source.origin);
        let q = css.apply(source.origin + point(source.size.width, source.size.height));
        let mask = ContentMask {
            bounds: Bounds {
                origin: point(p.x.min(q.x), p.y.min(q.y)),
                size: size((p.x - q.x).abs(), (p.y - q.y).abs()),
            }
            .intersect(&scope.parent.bounds),
        };
        window
            .with_content_mask_replaced(mask, |window| window.with_transformation_masked(matrix, f))
    } else {
        window.with_transformation_masked(matrix, f)
    }
}

fn source_clip(scope: Scope, current: Bounds<Pixels>, scale: f32) -> Bounds<Pixels> {
    let initial = super::fragment_mask::snap(scope.clip, scale)
        .bounds
        .intersect(&scope.parent.bounds);
    // Fragment clipping precedes effects and device snapping (CSS Break 3
    // §5.5). Mapping an already rounded edge doubles its phase error under
    // scaling. Recover CSS edges when this is the fragment's own clip; an
    // additional ancestor clip has no recoverable CSS geometry here.
    if current == initial {
        scope.clip
    } else {
        current
    }
}

#[cfg(test)]
mod tests {
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
        let owner = PaintCx::default();
        with(&owner, Some(scope(49.71875, 100.0)), || {
            let reference = reference_box(Some(&owner), bounds(-50.28125, 120.0)).unwrap();
            assert_eq!(reference.origin.y, px(49.71875));
            assert_eq!(reference.size.height, px(20.0));
        });
    }

    #[test]
    fn overflow_only_reference_box_is_empty_at_fragmentainer_start() {
        let owner = PaintCx::default();
        with(&owner, Some(scope(49.71875, 100.0)), || {
            for y in [-150.28125, 149.71875] {
                let reference = reference_box(Some(&owner), bounds(y, 77.0)).unwrap();
                assert_eq!(reference.origin.y, px(49.71875));
                assert_eq!(reference.size.height, px(0.0));
            }
        });
    }

    #[test]
    fn nested_scope_restores_the_outer_fragment_reference() {
        let owner = PaintCx::default();
        with(&owner, Some(scope(50.0, 100.0)), || {
            with(&owner, Some(scope(100.0, 50.0)), || {
                assert_eq!(
                    reference_box(Some(&owner), bounds(0.0, 200.0)).unwrap(),
                    bounds(100.0, 50.0)
                );
            });
            assert_eq!(
                reference_box(Some(&owner), bounds(0.0, 200.0)).unwrap(),
                bounds(50.0, 100.0)
            );
        });
        assert!(reference_box(Some(&owner), bounds(0.0, 200.0)).is_none());
    }
}
