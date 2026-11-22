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

pub(in crate::scene) fn primitive(
    primitive: &mut Primitive,
    offset: Point<ScaledPixels>,
    clip: Option<Bounds<ScaledPixels>>,
) {
    match primitive {
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
