//! SVG-outline контура рамки, отдельно от заливок и теней.

use crate::paint::background::shape_path::border_geometry::miter_limit;
use crate::paint::background::*;

/// Разметка контура `outline` вокруг `border-shape` (Blink
/// `BorderShapePainter::PaintOutline`, `border_shape_painter.cc:275-334`):
/// полоса между внешним контуром, отодвинутым на `off + width`, и им же,
/// отодвинутым на `off` (у одной фигуры — плюс половина обводки рамки,
/// `OuterPathWithOffset`). Полоса собирается маской: белая обводка вдвое
/// шире внешнего отступа, чёрная — внутреннего, чёрная заливка — нутро;
/// `double` — две полосы по трети толщины (`:313-328`). Холст и border-box —
/// как у `border_shape_ring_svg`.
pub fn border_shape_outline_svg(
    outer: (&str, [f32; 4]),
    single: bool,
    stroke: f32,
    off: f32,
    width: f32,
    double: bool,
    colour: crate::style::values::value::Color,
    bw: f32,
    bh: f32,
    dx: f32,
    dy: f32,
    cw: f32,
    ch: f32,
) -> Option<String> {
    if width <= 0.0 {
        return None;
    }
    let (raw, [t, r, b, l]) = outer;
    let (d, rule) = border_shape_path(raw, (bw + l + r).max(0.0), (bh + t + b).max(0.0))?;
    if d.is_empty() {
        return None;
    }
    let tr = format!("translate({} {})", dx - l, dy - t);
    let ml = miter_limit(raw);
    let half = if single { stroke / 2.0 } else { 0.0 };
    // Полоса [r_in, r_out] от контура наружу: отрицательный внутренний
    // радиус (контур вжат внутрь) — чёрная заливка уже съедает нутро,
    // обводка внутрь не нужна.
    let band = |r_in: f32, r_out: f32| -> String {
        let outer_s = if r_out > 0.0 {
            format!(
                r##"<path d="{d}" fill="#ffffff" fill-rule="{rule}" stroke="#ffffff" stroke-width="{}" stroke-miterlimit="{ml}"/>"##,
                r_out * 2.0
            )
        } else {
            format!(r##"<path d="{d}" fill="#ffffff" fill-rule="{rule}"/>"##)
        };
        let inner_s = if r_in > 0.0 {
            format!(
                r##"<path d="{d}" fill="#000000" fill-rule="{rule}" stroke="#000000" stroke-width="{}" stroke-miterlimit="{ml}"/>"##,
                r_in * 2.0
            )
        } else if r_in < 0.0 {
            // Контур внутри фигуры: нутро до него остаётся полосой.
            format!(
                r##"<path d="{d}" fill="#000000" fill-rule="{rule}"/><path d="{d}" fill="none" stroke="#ffffff" stroke-width="{}" stroke-miterlimit="{ml}"/>"##,
                -r_in * 2.0
            )
        } else {
            format!(r##"<path d="{d}" fill="#000000" fill-rule="{rule}"/>"##)
        };
        format!("{outer_s}{inner_s}")
    };
    let r_in = off + half;
    let r_out = off + half + width;
    let body = if double && (width / 3.0).round() >= 1.0 {
        let third = (width / 3.0).round();
        format!("{}{}", band(r_out - third, r_out), band(r_in, r_in + third))
    } else {
        band(r_in, r_out)
    };
    // Полоса `double` внешняя и внутренняя лежат в одной маске: внутренняя
    // чёрная заливка второй полосы не задевает первую — она не выходит за
    // r_in + third < r_out − third.
    Some(format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{cw}" height="{ch}"><mask id="ol" maskUnits="userSpaceOnUse" x="0" y="0" width="{cw}" height="{ch}"><g transform="{tr}">{body}</g></mask><rect width="{cw}" height="{ch}" fill="rgb({},{},{})" fill-opacity="{}" mask="url(#ol)"/></svg>"##,
        (colour.r * 255.0).round(),
        (colour.g * 255.0).round(),
        (colour.b * 255.0).round(),
        colour.a
    ))
}
