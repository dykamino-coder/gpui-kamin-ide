//! CSS Writing Modes 4 §9.1.2–3: center and compress a physical 1em composition.
//! Window composes inner·outer; final offsets therefore use physical glyph axes.
use gpui::{Bounds, Pixels, Radians, Size, TransformationMatrix, point, px, size};

pub(super) fn transform(
    bounds: Bounds<Pixels>,
    natural: Size<Pixels>,
    em: f32,
    compress: bool,
    scale_factor: f32,
) -> TransformationMatrix {
    let sx = if compress && natural.width > px(em) && natural.width > px(0.0) {
        em / f32::from(natural.width)
    } else {
        1.0
    };
    let offset = if compress {
        point(
            (px(em) - natural.width * sx) / 2.0,
            (px(em) - natural.height) / 2.0,
        )
    } else {
        point(px(0.0), px(0.0))
    };
    let center = bounds.origin + point(bounds.size.width / 2.0, bounds.size.height / 2.0);
    let dev = |value: Pixels| value.scale(scale_factor);
    // With no compression or centering, preserve the existing carrier's exact matrix.
    if sx == 1.0 && offset == point(px(0.0), px(0.0)) {
        return TransformationMatrix::unit()
            .translate(point(dev(center.x), dev(center.y)))
            .rotate(Radians(-std::f32::consts::FRAC_PI_2))
            .translate(point(dev(center.x) * -1.0, dev(center.y) * -1.0));
    }
    TransformationMatrix::unit()
        .translate(point(dev(offset.x), dev(offset.y)))
        .translate(point(dev(bounds.origin.x), dev(bounds.origin.y)))
        .scale(size(sx, 1.0))
        .translate(point(
            dev(bounds.origin.x) * -1.0,
            dev(bounds.origin.y) * -1.0,
        ))
        .translate(point(dev(center.x), dev(center.y)))
        .rotate(Radians(-std::f32::consts::FRAC_PI_2))
        .translate(point(dev(center.x) * -1.0, dev(center.y) * -1.0))
}

/// `X = cur⁻¹ · CU · cur`: the box above is laid out in the coordinates
/// already mapped by the enclosing vertical line (`cur`, pushed first).
/// GPUI nests `with_transformation` as `outer · inner` (css-transforms-1
/// §transform-rendering: the parent's matrix applies last), so the quarter
/// turn is conjugated into the line's local space; without an enclosing
/// transform (`cur` = unit) the matrix is returned untouched.
pub(super) fn in_local_space(
    cu: TransformationMatrix,
    cur: TransformationMatrix,
) -> TransformationMatrix {
    if cur == TransformationMatrix::unit() {
        return cu;
    }
    let [[a, b], [c, d]] = cur.rotation_scale;
    let det = a * d - b * c;
    if det.abs() < 1e-12 {
        return cu;
    }
    let [tx, ty] = cur.translation;
    let inv = TransformationMatrix {
        rotation_scale: [[d / det, -b / det], [-c / det, a / det]],
        translation: [(b * ty - d * tx) / det, (c * tx - a * ty) / det],
    };
    inv.compose(cu).compose(cur)
}
