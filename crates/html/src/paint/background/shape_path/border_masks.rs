//! Typed coverage masks and colored rings of border-shape contours.

use super::border_scene::Contour;
use crate::document::state::ValueCx;
use crate::svg::path_scene::*;

/// The nine-number header describes reference-box outsets, stroke width and
/// compositing outsets. Negative stroke contracts the inner overflow clip.
#[allow(clippy::too_many_arguments)]
pub(crate) fn border_shape_mask_scene(
    spec: &str,
    bw: f32,
    bh: f32,
    dx: f32,
    dy: f32,
    cw: f32,
    ch: f32,
    vcx: ValueCx<'_>,
) -> Option<Scene> {
    let (head, raw) = spec.split_once(':')?;
    let v: Vec<f32> = head
        .split_whitespace()
        .filter_map(|t| t.parse().ok())
        .collect();
    if v.len() != 9 {
        return None;
    }
    let contour = Contour::place((raw, [v[0], v[1], v[2], v[3]]), bw, bh, dx, dy, vcx)?;
    Some(Scene::new(cw, ch, contour.offset(true, v[4] / 2.0)))
}

/// Two shapes use ordered white/black luminance coverage: evenodd would also
/// paint the part of the inner shape outside the outer one.
#[allow(clippy::too_many_arguments)]
pub(crate) fn border_shape_ring_scene(
    outer: (&str, [f32; 4]),
    inner: Option<(&str, [f32; 4])>,
    stroke: f32,
    colour: crate::style::values::value::Color,
    bw: f32,
    bh: f32,
    dx: f32,
    dy: f32,
    cw: f32,
    ch: f32,
    vcx: ValueCx<'_>,
) -> Option<Scene> {
    let outer = Contour::place(outer, bw, bh, dx, dy, vcx)?;
    if outer.is_empty() {
        return None;
    }
    let root = if let Some(inner) = inner {
        let inner = Contour::place(inner, bw, bh, dx, dy, vcx)?;
        masked(
            cw,
            ch,
            paint(colour),
            group(vec![
                element(outer.path(Some(gray(true)), None)),
                element(inner.path(Some(gray(false)), None)),
            ]),
        )?
    } else {
        if stroke <= 0.0 {
            return None;
        }
        outer.path(None, Some((paint(colour), stroke)))
    };
    Some(Scene::new(cw, ch, root))
}
