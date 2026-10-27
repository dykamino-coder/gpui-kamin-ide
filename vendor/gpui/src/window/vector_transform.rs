//! Явная презентационная матрица воспроизведения записей (лист стопки страниц): векторные
//! команды переносятся точным подобием до растеризации и композиции. Вне scope запись
//! воспроизводится как прежде: преобразования вызывающего не применяются повторно.

use super::*;

/// Exact axis-preserving similarity in device pixels: positive uniform scale,
/// clockwise quarter-turns and a translation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct VectorTransform {
    scale: f32,
    quarter_turns: u8,
    translation: [f32; 2],
}

impl VectorTransform {
    /// Decompose a device-space matrix; `None` unless it is an exact similarity
    /// that keeps the axes (vector masks stay axis-aligned rectangles).
    pub(crate) fn from_matrix(m: TransformationMatrix) -> Option<Self> {
        let [[a, b], [c, d]] = m.rotation_scale;
        let near = |x: f32, y: f32| (x - y).abs() <= 1e-5 * x.abs().max(y.abs()).max(1.0);
        let (scale, quarter_turns) = if near(b, 0.) && near(c, 0.) && near(a, d) {
            (a.abs(), if a > 0. { 0 } else { 2 })
        } else if near(a, 0.) && near(d, 0.) && near(b, -c) {
            (c.abs(), if c > 0. { 1 } else { 3 })
        } else {
            return None;
        };
        (scale.is_finite() && scale > 0. && (1. / scale).is_finite()
            && m.translation.iter().all(|v| v.is_finite()))
            .then_some(Self { scale, quarter_turns, translation: m.translation })
    }

    /// Positive uniform scale.
    pub(crate) fn scale(self) -> f32 { self.scale }
    /// Number of clockwise quarter-turns, in `0..4`.
    pub(crate) fn quarter_turns(self) -> u8 { self.quarter_turns }

    pub(crate) fn matrix(self) -> TransformationMatrix {
        let s = self.scale;
        let rotation_scale = match self.quarter_turns {
            1 => [[0., -s], [s, 0.]], 2 => [[-s, 0.], [0., -s]],
            3 => [[0., s], [-s, 0.]], _ => [[s, 0.], [0., s]],
        };
        TransformationMatrix { rotation_scale, translation: self.translation }
    }

    pub(crate) fn inverse_matrix(self) -> TransformationMatrix {
        let s = 1.0 / self.scale;
        let rotation_scale = match self.quarter_turns {
            1 => [[0., s], [-s, 0.]], 2 => [[-s, 0.], [0., -s]],
            3 => [[0., -s], [s, 0.]], _ => [[s, 0.], [0., s]],
        };
        let mut inverse = TransformationMatrix { rotation_scale, translation: [0.; 2] };
        let p = inverse.apply(point(px(-self.translation[0]), px(-self.translation[1])));
        inverse.translation = [p.x.0, p.y.0];
        inverse
    }

    /// Exact image of an axis-aligned logical rectangle at device scale `density`.
    pub(crate) fn bounds(self, b: Bounds<Pixels>, density: f32) -> Bounds<Pixels> {
        let m = self.matrix();
        let a = m.apply(point(b.origin.x * density, b.origin.y * density));
        let z = m.apply(point(b.right() * density, b.bottom() * density));
        Bounds::new(
            point(a.x.min(z.x) / density, a.y.min(z.y) / density),
            size((z.x - a.x).abs() / density, (z.y - a.y).abs() / density),
        )
    }
}

/// Logical image of `b` under the device presentation `m`.
pub(crate) fn presented_bounds(m: TransformationMatrix, b: Bounds<Pixels>, density: f32) -> Bounds<Pixels> {
    if let Some(exact) = VectorTransform::from_matrix(m) {
        return exact.bounds(b, density);
    }
    let d = crate::scene::affine_transform::placed(b.scale(density), &m);
    Bounds::new(
        point(px(d.origin.x.0 / density), px(d.origin.y.0 / density)),
        size(px(d.size.width.0 / density), px(d.size.height.0 / density)),
    )
}

/// A frozen scene under the device presentation `m` (non-singular).
pub(crate) fn present(scene: Scene, m: TransformationMatrix) -> Scene {
    match (VectorTransform::from_matrix(m), m.inverse()) {
        (Some(exact), _) => scene.transform_vector(exact),
        (None, Some(inverse)) => scene.transform_affine(m, inverse),
        (None, None) => scene,
    }
}

/// Device-space presentation matrix (non-singular).
struct Presentation(TransformationMatrix);

impl Window {
    /// Replay immutable records inside `paint` under `matrix` (logical pixels): the
    /// presentation transform of a page sheet, or the transform of the copy that
    /// replays a record. Only `paint_plan` reads it; live paint and capture are
    /// unaffected, and without this scope a caller's transform is never applied.
    /// Nested scopes compose (outer after inner). An exact axis-preserving similarity
    /// moves every vector command exactly; any other affine matrix is applied with
    /// live-paint parity (primitives compose it like `with_transformation`, masks move
    /// only when the axes are kept, like `with_transformation_masked`). A singular or
    /// non-finite matrix enters nothing and returns `InvalidGeometry`.
    pub fn with_paint_presentation<R>(
        &mut self,
        matrix: TransformationMatrix,
        paint: impl FnOnce(&mut Self) -> R,
    ) -> Result<R, PaintRecordError> {
        let scale = self.scale_factor();
        let device = TransformationMatrix {
            rotation_scale: matrix.rotation_scale,
            translation: [matrix.translation[0] * scale, matrix.translation[1] * scale],
        };
        let device = match self.paint_presentation() {
            Some(outer) => outer.compose(device),
            None => device,
        };
        if !device.translation.iter().chain(device.rotation_scale.iter().flatten()).all(|v| v.is_finite())
            || device.inverse().is_none()
        {
            return Err(PaintRecordError::InvalidGeometry);
        }
        let _scope = self.render_contexts.enter(Some(Rc::new(Presentation(device))));
        Ok(paint(self))
    }

    pub(crate) fn paint_presentation(&self) -> Option<TransformationMatrix> {
        self.render_contexts.get::<Presentation>().map(|value| value.0)
    }
}