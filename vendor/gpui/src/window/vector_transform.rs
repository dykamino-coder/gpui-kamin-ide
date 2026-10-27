//! Внешняя область векторного преобразования страниц до растеризации и композиции.

use super::*;

/// Uniform positive scale and an exact clockwise quarter-turn in logical pixels.
/// This presentation transform is deliberately separate from `PaintPlacement`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaintVectorTransform {
    scale: f32,
    quarter_turns: u8,
    translation: Point<Pixels>,
}

impl PaintVectorTransform {
    /// Construct a finite, invertible page presentation transform.
    pub fn new(scale: f32, quarter_turns: u8, translation: Point<Pixels>) -> Option<Self> {
        (scale.is_finite() && scale > 0.0 && (1.0 / scale).is_finite()
            && translation.x.0.is_finite() && translation.y.0.is_finite())
            .then_some(Self { scale, quarter_turns: quarter_turns % 4, translation })
    }

    /// Positive uniform scale.
    pub fn scale(self) -> f32 { self.scale }
    /// Number of clockwise quarter-turns, in `0..4`.
    pub fn quarter_turns(self) -> u8 { self.quarter_turns }
    /// Translation in destination logical pixels.
    pub fn translation(self) -> Point<Pixels> { self.translation }
    /// Map a source point to the destination.
    pub fn point(self, p: Point<Pixels>) -> Point<Pixels> { self.matrix(1.0).apply(p) }
    /// Map destination coordinates back to the source.
    pub fn inverse_point(self, p: Point<Pixels>) -> Point<Pixels> { self.inverse_matrix(1.0).apply(p) }
    /// Map an axis-aligned source rectangle exactly.
    pub fn bounds(self, b: Bounds<Pixels>) -> Bounds<Pixels> { map_bounds(b, self.matrix(1.0)) }
    /// Obtain an exact source rectangle for a destination clip.
    pub fn inverse_bounds(self, b: Bounds<Pixels>) -> Bounds<Pixels> { map_bounds(b, self.inverse_matrix(1.0)) }

    pub(crate) fn matrix(self, density: f32) -> TransformationMatrix {
        let s = self.scale;
        let rotation_scale = match self.quarter_turns {
            1 => [[0., -s], [s, 0.]], 2 => [[-s, 0.], [0., -s]],
            3 => [[0., s], [-s, 0.]], _ => [[s, 0.], [0., s]],
        };
        TransformationMatrix { rotation_scale,
            translation: [self.translation.x.0 * density, self.translation.y.0 * density] }
    }

    pub(crate) fn inverse_matrix(self, density: f32) -> TransformationMatrix {
        self.inverse_of(self.matrix(density))
    }

    pub(crate) fn inverse_of(self, m: TransformationMatrix) -> TransformationMatrix {
        let s = 1.0 / self.scale;
        let rotation_scale = match self.quarter_turns {
            1 => [[0., s], [-s, 0.]], 2 => [[-s, 0.], [0., -s]],
            3 => [[0., -s], [s, 0.]], _ => [[s, 0.], [0., s]],
        };
        let mut inverse = TransformationMatrix {
            rotation_scale,
            translation: [0.; 2],
        };
        let p = inverse.apply(point(px(-m.translation[0]), px(-m.translation[1])));
        inverse.translation = [p.x.0, p.y.0];
        inverse
    }
}

fn map_bounds(b: Bounds<Pixels>, m: TransformationMatrix) -> Bounds<Pixels> {
    let a = m.apply(b.origin);
    let z = m.apply(b.bottom_right());
    Bounds::new(point(a.x.min(z.x), a.y.min(z.y)), size((z.x - a.x).abs(), (z.y - a.y).abs()))
}

impl Window {
    /// Transform resolved vector commands, their clips and flat composition groups.
    /// No rendered image is resized; immutable records remain translation-only.
    /// The callback uses source coordinates, independently of the ambient CSS matrix.
    pub fn with_paint_vector_transform<R>(
        &mut self,
        transform: PaintVectorTransform,
        paint: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.invalidator.debug_assert_paint();
        let state = Box::new(self.snapshot_paint_state());
        let source_clip = transform.inverse_bounds(self.content_mask().bounds);
        let (result, scene) = self.with_restore(state, |window, state| state.restore(window), |window| {
            window.transformation_stack.clear();
            window.mask_map = None;
            window.mask_scale = None;
            window.content_mask_stack = vec![ContentMask { bounds: source_clip }];
            window.with_isolated_scene(paint)
        }).0;
        let scene = scene.transform_vector(transform, self.scale_factor());
        let scene = self.adopt_scene(scene);
        self.next_frame.scene.replay(0..scene.paint_operations.len(), &scene);
        result
    }
}
