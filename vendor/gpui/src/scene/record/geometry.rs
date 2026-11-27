//! Перенос разрешённой геометрии команд без повторной растеризации текста.

use super::*;

pub(in crate::scene) fn bounds(
    mut bounds: Bounds<ScaledPixels>,
    offset: Point<ScaledPixels>,
) -> Bounds<ScaledPixels> {
    bounds.origin += offset;
    bounds
}

pub(super) fn mask(
    mask: &mut ContentMask<ScaledPixels>,
    offset: Point<ScaledPixels>,
    clip: Option<Bounds<ScaledPixels>>,
) {
    mask.bounds = bounds(mask.bounds, offset);
    if let Some(clip) = clip {
        mask.bounds = mask.bounds.intersect(&clip);
    }
}

fn transform(matrix: &mut TransformationMatrix, offset: Point<ScaledPixels>) {
    matrix.translation[0] += offset.x.0;
    matrix.translation[1] += offset.y.0;
}

/// Keep shader-local coordinates equal to a fresh paint at the new offset.
/// Moving only the output translation subtracts large scroll coordinates
/// when the gradient shader maps a sample back to the primitive's bounds.
/// Move the local bounds too and conjugate the matrix: T(d) M T(-d).
fn shifted_geometry(
    rect: &mut Bounds<ScaledPixels>,
    matrix: &mut TransformationMatrix,
    offset: Point<ScaledPixels>,
) {
    if matrix.rotation_scale == [[1.0, 0.0], [0.0, 1.0]] {
        transform(matrix, offset);
        return;
    }
    rect.origin += offset;
    let [x, y] = [offset.x.0, offset.y.0];
    let m = matrix.rotation_scale;
    matrix.translation[0] += (1.0 - m[0][0]) * x - m[0][1] * y;
    matrix.translation[1] += -m[1][0] * x + (1.0 - m[1][1]) * y;
}

/// A scroll move of a marked primitive by `offset` (`scroll_marks`): an
/// identity matrix moves with its translation, as `primitive` moves it;
/// another is built again from its `pivot`, moved, by the function a fresh
/// paint at the new offset runs, so both give the same bits. Returns the
/// pivot of the moved geometry. A marked primitive without the pivot its
/// matrix needs refused the move (`ScrollRefusal::Transform`).
pub(in crate::scene) fn scrolled(
    primitive: &mut Primitive,
    offset: Point<ScaledPixels>,
    pivot: Option<&TransformPivot>,
) -> Option<TransformPivot> {
    if offset == Point::default() {
        return pivot.cloned();
    }
    let exact = primitive.transformation().is_none_or(TransformationMatrix::moves_exactly);
    let Some(moved) = pivot.map(|pivot| pivot.moved(offset)).filter(|_| !exact) else {
        debug_assert!(exact, "a scroll move of a matrix without its pivot");
        self::primitive(primitive, offset, None);
        return pivot.map(|pivot| pivot.moved(offset));
    };
    let (rect, matrix) = match primitive {
        Primitive::Gradient(g) => (&mut g.quad.bounds, &mut g.quad.transformation),
        Primitive::Quad(p) => (&mut p.bounds, &mut p.transformation),
        Primitive::MonochromeSprite(p) => (&mut p.bounds, &mut p.transformation),
        Primitive::SubpixelSprite(p) => (&mut p.bounds, &mut p.transformation),
        Primitive::PolychromeSprite(p) => (&mut p.bounds, &mut p.transformation),
        _ => unreachable!("a primitive without a matrix moves exactly"),
    };
    rect.origin += offset;
    *matrix = moved.matrix();
    mask(primitive.content_mask_mut(), offset, None);
    Some(moved)
}

pub(in crate::scene) fn primitive(
    primitive: &mut Primitive,
    offset: Point<ScaledPixels>,
    clip: Option<Bounds<ScaledPixels>>,
) {
    match primitive {
        Primitive::Gradient(g) => {
            let p = &mut g.quad;
            shifted_geometry(&mut p.bounds, &mut p.transformation, offset);
            mask(&mut p.content_mask, offset, clip);
        }
        Primitive::Quad(p) => {
            shifted_geometry(&mut p.bounds, &mut p.transformation, offset);
            mask(&mut p.content_mask, offset, clip);
        }
        Primitive::MonochromeSprite(p) => {
            shifted_geometry(&mut p.bounds, &mut p.transformation, offset);
            mask(&mut p.content_mask, offset, clip);
        }
        Primitive::SubpixelSprite(p) => {
            shifted_geometry(&mut p.bounds, &mut p.transformation, offset);
            mask(&mut p.content_mask, offset, clip);
        }
        Primitive::PolychromeSprite(p) => {
            shifted_geometry(&mut p.bounds, &mut p.transformation, offset);
            mask(&mut p.content_mask, offset, clip);
        }
        Primitive::Shadow(p) => {
            p.bounds = bounds(p.bounds, offset);
            p.element_bounds = bounds(p.element_bounds, offset);
            mask(&mut p.content_mask, offset, clip);
        }
        Primitive::Underline(p) => {
            p.bounds = bounds(p.bounds, offset);
            mask(&mut p.content_mask, offset, clip);
        }
        Primitive::Surface(p) => {
            p.bounds = bounds(p.bounds, offset);
            mask(&mut p.content_mask, offset, clip);
        }
        Primitive::Path(p) => {
            if let Some((_, inverse)) = &mut p.color_mapping {
                *inverse = inverse.compose(
                    TransformationMatrix::unit()
                        .translate(point(ScaledPixels(-offset.x.0), ScaledPixels(-offset.y.0))),
                );
            }
            p.bounds = bounds(p.bounds, offset);
            p.start += offset;
            p.current += offset;
            mask(&mut p.content_mask, offset, clip);
            for vertex in &mut p.vertices {
                vertex.xy_position += offset;
                mask(&mut vertex.content_mask, offset, clip);
            }
        }
    }
}

/// Перенос затрагивает координаты кадра и буфера: H' = T H T^-1.
pub(super) fn projection(m: [f32; 9], offset: Point<ScaledPixels>) -> [f32; 9] {
    let (x, y) = (offset.x.0, offset.y.0);
    let a = m[0] + x * m[6];
    let b = m[1] + x * m[7];
    let c = m[2] + x * m[8];
    let d = m[3] + y * m[6];
    let e = m[4] + y * m[7];
    let f = m[5] + y * m[8];
    [
        a,
        b,
        c - a * x - b * y,
        d,
        e,
        f - d * x - e * y,
        m[6],
        m[7],
        m[8] - m[6] * x - m[7] * y,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{px, size};

    #[test]
    fn scrolled_skew_keeps_the_fresh_local_coordinates() {
        let rect = Bounds {
            origin: point(ScaledPixels(49.0), ScaledPixels(486.02)),
            size: size(ScaledPixels(702.0), ScaledPixels(220.0)),
        };
        let matrix = TransformationMatrix {
            rotation_scale: [[1.0, 0.0], [-0.14054084, 1.0]],
            translation: [0.0, 56.216335],
        };
        let mut command = Primitive::Quad(Quad {
            bounds: rect,
            transformation: matrix,
            ..Default::default()
        });
        let offset = point(ScaledPixels(0.0), ScaledPixels(-1680.0));
        primitive(&mut command, offset, None);
        let Primitive::Quad(quad) = command else {
            unreachable!()
        };
        assert_eq!(quad.bounds.origin, rect.origin + offset);
        // The transform origin's x is unchanged by vertical scrolling:
        // a fresh skewY has this exact matrix, without -1680 in its inverse.
        assert_eq!(quad.transformation, matrix);
    }

    /// A scroll move conjugating the finished `f32` matrix rounds again and
    /// misses the fresh bits; the pivot builds them.
    #[test]
    fn a_scrolled_rotation_takes_the_fresh_bits_from_its_pivot() {
        let (c, s) = (0.9575, 0.2883);
        let linear = [[c, -s], [s, c]];
        let pivot_at = |y: f32| TransformPivot::new(linear, [0.0; 2], (point(px(73.0), px(y)), [27.0, 42.0]), 1.0, (None, None));
        let mut conjugation_missed = 0;
        for d in [-40.0_f32, -120.0, -360.0, -1680.0, 80.0] {
            let pivot = pivot_at(486.0);
            let quad = |matrix| Primitive::Quad(Quad {
                bounds: Bounds { origin: point(ScaledPixels(73.0), ScaledPixels(486.0)), size: size(ScaledPixels(90.0), ScaledPixels(60.0)) },
                transformation: matrix,
                ..Default::default()
            });
            let offset = point(ScaledPixels(0.0), ScaledPixels(d));
            let fresh = pivot_at(486.0 + d).matrix();
            let mut conjugated = quad(pivot.matrix());
            primitive(&mut conjugated, offset, None);
            let Primitive::Quad(conjugated) = conjugated else { unreachable!() };
            conjugation_missed += (conjugated.transformation.translation.map(f32::to_bits) != fresh.translation.map(f32::to_bits)) as usize;
            let mut moved = quad(pivot.matrix());
            let held = scrolled(&mut moved, offset, Some(&pivot));
            let Primitive::Quad(moved) = moved else { unreachable!() };
            assert_eq!(moved.transformation.translation.map(f32::to_bits), fresh.translation.map(f32::to_bits), "step {d}");
            assert_eq!(moved.bounds.origin.y.0, 486.0 + d);
            assert_eq!(held.map(|held| held.matrix()), Some(fresh));
        }
        assert!(conjugation_missed > 0, "the conjugated matrix should miss the fresh bits");
    }

    #[test]
    fn rebasing_a_rotated_quad_preserves_its_screen_corners() {
        let mut rect = Bounds {
            origin: point(ScaledPixels(24.0), ScaledPixels(40.0)),
            size: size(ScaledPixels(12.0), ScaledPixels(30.0)),
        };
        let original = TransformationMatrix {
            rotation_scale: [[0.0, -2.0], [3.0, 0.0]],
            translation: [150.0, -20.0],
        };
        let mut matrix = original;
        let offset = point(ScaledPixels(-7.0), ScaledPixels(-120.0));
        let before = [rect.origin, rect.bottom_right()];
        shifted_geometry(&mut rect, &mut matrix, offset);
        for (before, after) in before.into_iter().zip([rect.origin, rect.bottom_right()]) {
            let unscale = |p: Point<ScaledPixels>| point(px(p.x.0), px(p.y.0));
            assert_eq!(
                matrix.apply(unscale(after)),
                original.apply(unscale(before)) + unscale(offset)
            );
        }
    }
}
