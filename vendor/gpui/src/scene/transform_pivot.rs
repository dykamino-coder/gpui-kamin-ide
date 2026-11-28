//! KaminIDE patch: the inputs of a flat CSS transform matrix, so a scrolled
//! replay builds the moved matrix as a fresh paint at the new offset does.
//!
//! The matrix translation of a rotated, skewed or scaled box depends on
//! where its origin lies: `t + (I - M) origin`, rounded once to `f32`, and a
//! quarter turn then snaps its edges to the device grid from where the box
//! lies. Moving the finished `f32` matrix (`T(d) M T(-d)`) rounds again, so
//! its bits differ from the fresh matrix and the edges and gradients of such
//! a box land on other pixels. A pivot keeps the inputs instead: the paint
//! and the replay both build the matrix with `TransformPivot::new`, the
//! replay from the moved inputs, so both give the same bits.

use super::{Point, ScaledPixels, TransformationMatrix};
use crate::{Bounds, Pixels};
use std::rc::Rc;

/// A flat transform around its origin, as the painter resolved it.
#[derive(Clone, Debug)]
pub struct TransformPivot {
    linear: [[f32; 2]; 2],
    /// The translation part, in device pixels (percentages resolved).
    translation: [f32; 2],
    /// The box origin the transform origin is offset from (logical).
    base: Point<Pixels>,
    /// The transform origin from `base` (logical).
    offset: [f32; 2],
    scale_factor: f32,
    snap: Option<QuarterSnap>,
    /// The pivot of the transform this one is painted under, if any.
    outer: Option<Rc<TransformPivot>>,
    /// The matrix before an outer transform composes it.
    local: TransformationMatrix,
    /// The quarter turn's exact fill matrix (`fill`).
    fill: Option<TransformationMatrix>,
    /// `outer · local` (`unit · local` with none), as primitives painted
    /// under it hold it (`Window::with_transformation_masked`).
    built: TransformationMatrix,
    /// The painter's own function of where the box lies, when it builds the
    /// matrix itself (`built_by`): the fields above then hold no inputs.
    build: Option<Rc<dyn TransformBuild>>,
    pub(crate) motion: Option<crate::ScrollMotion>,
}

/// A matrix the painter builds by its own function of where the box lies (a
/// 3D transform flattened to the screen): a scroll move builds it again from
/// the moved inputs, as a fresh paint at the new offset does.
pub trait TransformBuild: std::fmt::Debug {
    /// The matrix painted under (an outer transform composes it).
    fn matrix(&self) -> TransformationMatrix;
    /// The same inputs for the box moved by `delta` (logical pixels).
    fn moved(&self, delta: Point<Pixels>) -> Rc<dyn TransformBuild>;
}

impl PartialEq for TransformPivot {
    /// Equal inputs (a painter's build by the matrix it builds).
    fn eq(&self, other: &Self) -> bool {
        (self.linear, self.translation, self.base, self.offset, self.scale_factor, self.snap)
            == (other.linear, other.translation, other.base, other.offset, other.scale_factor, other.snap)
            && (&self.outer, self.local, self.fill, self.built) == (&other.outer, other.local, other.fill, other.built)
            && self.build.is_some() == other.build.is_some()
    }
}

/// What a quarter turn (or reflection) snaps its edges from: an axis-aligned
/// box keeps its edges on the device grid as the same box laid out in place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QuarterSnap {
    /// The box as laid out (snapped to the device).
    pub bounds: Bounds<Pixels>,
    /// Where the box lies before device snapping.
    pub exact_origin: Point<Pixels>,
    /// Its size before device snapping.
    pub exact_size: [f32; 2],
    /// The origin the fill matrix is exact from (CSS Transforms 1 §3).
    pub raw_origin: Point<Pixels>,
}

impl TransformPivot {
    /// The pivot of `linear` and `translation` around `base + offset`,
    /// painted under `outer`; a quarter turn also snaps its edges from `snap`.
    pub fn new(
        linear: [[f32; 2]; 2],
        translation: [f32; 2],
        (base, offset): (Point<Pixels>, [f32; 2]),
        scale_factor: f32,
        (snap, outer): (Option<QuarterSnap>, Option<Rc<TransformPivot>>),
    ) -> Self {
        let dev = |v: f32| Pixels(v).scale(scale_factor);
        let origin = Point { x: dev(base.x.0 + offset[0]), y: dev(base.y.0 + offset[1]) };
        let mut local = TransformationMatrix::around_origin(linear, translation, origin);
        let fill = snap.map(|snap| exact_fill(local, snap.bounds.origin, snap.raw_origin, scale_factor));
        if let Some(snap) = snap {
            snap_quarter(&mut local, snap, scale_factor);
        }
        let built = outer.as_ref().map_or(TransformationMatrix::unit(), |outer| outer.built).compose(local);
        TransformPivot { linear, translation, base, offset, scale_factor, snap, outer, local, fill, built, build: None, motion: None }
    }

    /// The pivot of a matrix `build` makes, painted under `outer`.
    pub fn built_by(build: Rc<dyn TransformBuild>, scale_factor: f32, outer: Option<Rc<TransformPivot>>) -> Self {
        let local = build.matrix();
        let built = outer.as_ref().map_or(TransformationMatrix::unit(), |outer| outer.built).compose(local);
        let (linear, translation, base, offset) = ([[1.0, 0.0], [0.0, 1.0]], [0.0; 2], Point::default(), [0.0; 2]);
        TransformPivot { linear, translation, base, offset, scale_factor, snap: None, outer, local, fill: None, built, build: Some(build), motion: None }
    }

    /// The matrix to paint under (an outer transform composes it).
    pub fn local(&self) -> TransformationMatrix {
        self.local
    }

    /// The exact fill matrix of a quarter turn.
    pub fn fill(&self) -> Option<TransformationMatrix> {
        self.fill
    }

    /// The matrix primitives painted under this pivot hold.
    pub fn matrix(&self) -> TransformationMatrix {
        self.built
    }

    /// Resolve each ancestor at its own spatial location, including nested scrolls.
    pub fn current_matrix(&self) -> TransformationMatrix {
        let mut local = self.clone();
        local.outer = None;
        let delta = self.motion.as_ref().map_or(Point::default(), |motion| motion.delta());
        let moved = local.moved(delta.map(|v| v.scale(self.scale_factor)));
        self.outer.as_ref().map_or(TransformationMatrix::unit(), |outer| outer.current_matrix()).compose(moved.local)
    }

    /// The pivot of the same box moved by `offset` (device pixels): every
    /// place it is built from moves, in logical pixels, before it is built.
    pub(crate) fn moved(&self, offset: Point<ScaledPixels>) -> Self {
        if offset == Point::default() {
            return self.clone();
        }
        let delta = Point { x: Pixels(offset.x.0 / self.scale_factor), y: Pixels(offset.y.0 / self.scale_factor) };
        let shift = |p: Point<Pixels>| Point { x: Pixels(p.x.0 + delta.x.0), y: Pixels(p.y.0 + delta.y.0) };
        let snap = self.snap.map(|snap| QuarterSnap {
            bounds: Bounds { origin: shift(snap.bounds.origin), size: snap.bounds.size },
            exact_origin: shift(snap.exact_origin),
            raw_origin: shift(snap.raw_origin),
            ..snap
        });
        let outer = self.outer.as_ref().map(|outer| Rc::new(outer.moved(offset)));
        if let Some(build) = &self.build {
            return Self::built_by(build.moved(delta), self.scale_factor, outer);
        }
        Self::new(self.linear, self.translation, (shift(self.base), self.offset), self.scale_factor, (snap, outer))
    }
}

/// The fill matrix of a quarter turn: origins resolve before device snapping
/// (CSS Transforms 1 §3), so box fills snap both edges from this map while
/// glyphs and images keep the snapped one.
fn exact_fill(mut matrix: TransformationMatrix, snapped: Point<Pixels>, exact: Point<Pixels>, sf: f32) -> TransformationMatrix {
    let [[a, b], [c, d]] = matrix.rotation_scale;
    let dx = (exact.x.0 - snapped.x.0) * sf;
    let dy = (exact.y.0 - snapped.y.0) * sf;
    matrix.translation[0] += (1.0 - a) * dx - b * dy;
    matrix.translation[1] += -c * dx + (1.0 - d) * dy;
    matrix
}

/// A quarter turn (or reflection) keeps the box axis-aligned: its edges
/// round to the device grid as the same box laid out in place does (layout
/// rounds half up). The rasterizer's top-left rule would otherwise settle an
/// exact half by `f32` noise (`rotate(-90deg)` has cos ≈ -4e-8); Skia lays an
/// axis-aligned rect by round(x) (`SkScan::FillRect`).
fn snap_quarter(matrix: &mut TransformationMatrix, snap: QuarterSnap, sf: f32) {
    let corner_min = |o: Point<Pixels>, w: f32, h: f32| {
        let mut m = (f32::INFINITY, f32::INFINITY);
        for (dx, dy) in [(0.0, 0.0), (w, 0.0), (0.0, h), (w, h)] {
            let p = matrix.apply(Point { x: Pixels((o.x.0 + dx) * sf), y: Pixels((o.y.0 + dy) * sf) });
            m = (m.0.min(p.x.0), m.1.min(p.y.0));
        }
        m
    };
    let exact = corner_min(snap.exact_origin, snap.exact_size[0], snap.exact_size[1]);
    let cur = corner_min(snap.bounds.origin, snap.bounds.size.width.0, snap.bounds.size.height.0);
    let round = |v: f32| ((v * 64.0).round() / 64.0 + 0.5).floor();
    matrix.translation[0] += round(exact.0) - cur.0;
    matrix.translation[1] += round(exact.1) - cur.1;
}

impl TransformationMatrix {
    /// KaminIDE patch: `t + (I - M) origin`, computed in `f64` and rounded
    /// once to the raster's `f32` matrix. Composing three `f32` matrices
    /// first adds and then subtracts the origin, losing bits that vary with
    /// the scroll position; Blink likewise keeps double precision in
    /// `AffineTransform` until the raster boundary.
    pub fn around_origin(linear: [[f32; 2]; 2], translation: [f32; 2], origin: Point<ScaledPixels>) -> Self {
        let [x, y] = [f64::from(origin.x.0), f64::from(origin.y.0)];
        TransformationMatrix {
            rotation_scale: linear,
            translation: [
                (f64::from(translation[0]) + (1.0 - f64::from(linear[0][0])) * x - f64::from(linear[0][1]) * y) as f32,
                (f64::from(translation[1]) - f64::from(linear[1][0]) * x + (1.0 - f64::from(linear[1][1])) * y) as f32,
            ],
        }
    }

    /// KaminIDE patch: whether a scroll move may conjugate this matrix
    /// (`T(d) M T(-d)`): only the identity, whose move is its translation's.
    /// Every other matrix is built again from its pivot.
    pub(crate) fn moves_exactly(&self) -> bool {
        self.rotation_scale == [[1.0, 0.0], [0.0, 1.0]]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{point, px, size};

    #[test]
    fn a_moved_pivot_builds_the_fresh_matrix_bit_for_bit() {
        let (c, s) = (0.2588190_f32, 0.9659258_f32);
        let snap = |at: Point<Pixels>| QuarterSnap {
            bounds: Bounds { origin: at, size: size(px(90.0), px(60.0)) },
            exact_origin: point(at.x + px(0.25), at.y),
            exact_size: [90.0, 60.5],
            raw_origin: at,
        };
        for (linear, quarter) in [([[c, -s], [s, c]], false), ([[0.0, -1.0], [1.0, 0.0]], true)] {
            for scroll in [-40.0_f32, -1680.0, 3.0, 65_000.0] {
                let at = point(px(113.0), px(486.0));
                let to = point(at.x, at.y + px(scroll));
                let outer = |at: Point<Pixels>| Rc::new(TransformPivot::new([[c, s], [-s, c]], [1.5, 0.0], (at, [3.0, 9.0]), 2.0, (None, None)));
                let nested = |at: Point<Pixels>| (!quarter).then(|| outer(at));
                let pivot = TransformPivot::new(linear, [4.0, -2.5], (at, [37.5, 12.25]), 2.0, (quarter.then(|| snap(at)), nested(at)));
                let moved = pivot.moved(point(ScaledPixels(0.0), ScaledPixels(scroll * 2.0)));
                let fresh = TransformPivot::new(linear, [4.0, -2.5], (to, [37.5, 12.25]), 2.0, (quarter.then(|| snap(to)), nested(to)));
                assert_eq!(moved.matrix().translation.map(f32::to_bits), fresh.matrix().translation.map(f32::to_bits));
                assert_eq!(moved, fresh);
            }
        }
    }

    #[test]
    fn origin_and_translation_apply_in_css_order() {
        let matrix = TransformationMatrix::around_origin(
            [[0.0, -1.0], [1.0, 0.0]],
            [7.0, -3.0],
            point(ScaledPixels(40.0), ScaledPixels(20.0)),
        );
        assert_eq!(matrix.apply(point(px(45.0), px(20.0))), point(px(47.0), px(22.0)));
    }

    #[test]
    fn scrolling_along_a_skews_preserved_axis_loses_no_origin_bits() {
        let linear = [[1.0, 0.0], [-0.14054084, 1.0]];
        let mut expected = None;
        for y in [486.02, -1193.98, 791.02, 65_536.0, -65_536.0] {
            let matrix = TransformationMatrix::around_origin(linear, [0.0; 2], point(ScaledPixels(400.0), ScaledPixels(y)));
            assert_eq!(matrix.translation, [0.0, 56.216335]);
            if let Some(expected) = expected {
                assert_eq!(matrix, expected);
            }
            expected = Some(matrix);
        }
    }
}
