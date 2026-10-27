//! Перенос разрешённой геометрии команд без повторной растеризации текста.

use super::*;

pub(in crate::scene) fn bounds(mut bounds: Bounds<ScaledPixels>, offset: Point<ScaledPixels>) -> Bounds<ScaledPixels> {
    bounds.origin += offset;
    bounds
}

pub(super) fn mask(mask: &mut ContentMask<ScaledPixels>, offset: Point<ScaledPixels>, clip: Option<Bounds<ScaledPixels>>) {
    mask.bounds = bounds(mask.bounds, offset);
    if let Some(clip) = clip {
        mask.bounds = mask.bounds.intersect(&clip);
    }
}

fn transform(matrix: &mut TransformationMatrix, offset: Point<ScaledPixels>) {
    matrix.translation[0] += offset.x.0;
    matrix.translation[1] += offset.y.0;
}

pub(in crate::scene) fn primitive(primitive: &mut Primitive, offset: Point<ScaledPixels>, clip: Option<Bounds<ScaledPixels>>) {
    match primitive {
        Primitive::Quad(p) => {
            transform(&mut p.transformation, offset);
            mask(&mut p.content_mask, offset, clip);
        }
        Primitive::MonochromeSprite(p) => {
            transform(&mut p.transformation, offset);
            mask(&mut p.content_mask, offset, clip);
        }
        Primitive::SubpixelSprite(p) => {
            transform(&mut p.transformation, offset);
            mask(&mut p.content_mask, offset, clip);
        }
        Primitive::PolychromeSprite(p) => {
            transform(&mut p.transformation, offset);
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
                *inverse = inverse.compose(TransformationMatrix::unit().translate(-offset));
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
    [a, b, c - a * x - b * y,
     d, e, f - d * x - e * y,
     m[6], m[7], m[8] - m[6] * x - m[7] * y]
}
