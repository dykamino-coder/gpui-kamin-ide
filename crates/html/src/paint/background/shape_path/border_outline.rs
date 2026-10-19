//! Border-shape outline bands built from ordered luminance coverage paths.

use super::border_scene::Contour;
use crate::document::state::ValueCx;
use crate::svg::path_scene::*;

/// Blink BorderShapePainter::PaintOutline: offset the outer path, then
/// subtract its inner offset. Double outlines paint both bands in one mask.
#[allow(clippy::too_many_arguments)]
pub(crate) fn border_shape_outline_scene(
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
    vcx: ValueCx<'_>,
) -> Option<Scene> {
    if width <= 0.0 {
        return None;
    }
    let contour = Contour::place(outer, bw, bh, dx, dy, vcx)?;
    if contour.is_empty() {
        return None;
    }
    let half = if single { stroke / 2.0 } else { 0.0 };
    let (inside, outside) = (off + half, off + half + width);
    let third = (width / 3.0).round();
    let bands = if double && third >= 1.0 {
        vec![(outside - third, outside), (inside, inside + third)]
    } else {
        vec![(inside, outside)]
    };
    let mut children = Vec::new();
    for (a, b) in bands {
        children.push(element(contour.offset(true, b.max(0.0))));
        children.push(element(contour.offset(false, a)));
    }
    Some(Scene::new(
        cw,
        ch,
        masked(cw, ch, paint(colour), group(children))?,
    ))
}
