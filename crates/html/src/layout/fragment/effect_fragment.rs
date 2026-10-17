//! CSS Break 3 §5.5: graphical effects use each fragment's reference box.
//!
//! The continuous copy still owns layout and containing-block geometry. Only
//! the effect reference box and the fragment's source clip change at paint.

use gpui::{Bounds, ContentMask, Pixels, TransformationMatrix, Window, point, px, size};
use std::cell::RefCell;

#[derive(Clone, Copy)]
pub(crate) struct Scope {
    pub parent: ContentMask<Pixels>,
    pub fragment: Bounds<Pixels>,
    pub clip: Bounds<Pixels>,
}

thread_local! {
    static ACTIVE: RefCell<Option<Scope>> = const { RefCell::new(None) };
}

pub(crate) fn with<R>(scope: Option<Scope>, f: impl FnOnce() -> R) -> R {
    let previous = ACTIVE.with(|active| active.replace(scope));
    let result = f();
    ACTIVE.with(|active| active.replace(previous));
    result
}

/// Краска идёт внутри фрагмента внешней фрагментации.
pub(crate) fn active() -> bool {
    ACTIVE.with(|active| active.borrow().is_some())
}

pub(crate) fn reference_box(bounds: Bounds<Pixels>) -> Option<Bounds<Pixels>> {
    let scope = ACTIVE.with(|active| *active.borrow())?;
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
    bounds: Bounds<Pixels>,
    ink: (Pixels, Pixels),
    window: &mut Window,
    f: impl FnOnce(&mut Window) -> R,
) -> R {
    let Some(scope) = ACTIVE.with(|active| *active.borrow()) else {
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
    matrix: TransformationMatrix,
    clip_matrix: TransformationMatrix,
    clip_ancestor: bool,
    window: &mut Window,
    f: impl FnOnce(&mut Window) -> R,
) -> R {
    let scope = ACTIVE.with(|active| *active.borrow());
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
mod tests;
