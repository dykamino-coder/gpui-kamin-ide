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

#[cfg(test)]
mod tests;

pub(crate) fn paint<R>(
    matrix: TransformationMatrix,
    window: &mut Window,
    f: impl FnOnce(&mut Window) -> R,
) -> R {
    let scope = ACTIVE.with(|active| *active.borrow());
    let [[a, b], [c, d]] = matrix.rotation_scale;
    let axes = (b.abs() < 1e-4 && c.abs() < 1e-4) || (a.abs() < 1e-4 && d.abs() < 1e-4);
    if let Some(scope) = scope.filter(|_| axes) {
        let mut css = matrix;
        for v in &mut css.translation {
            *v /= window.scale_factor();
        }
        let source = window.content_mask().bounds;
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
