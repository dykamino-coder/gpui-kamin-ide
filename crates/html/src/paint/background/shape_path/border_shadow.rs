//! Border-shape shadows composed from typed paths, luminance masks and blur.

use super::border_scene::Contour;
use crate::svg::path_scene::*;

/// css-borders-4 border-shape-shadow-interaction: expand/contract the caster,
/// blur at sigma=blur/2, then cut the border. Ordered mask paints preserve
/// coverage at overlapping fill/stroke edges and translucent shadow colors.
#[allow(clippy::too_many_arguments)]
pub(crate) fn border_shape_shadow_scene(
    outer: (&str, [f32; 4]),
    inner: Option<(&str, [f32; 4])>,
    stroke: f32,
    shadows: &[(
        crate::style::computed::Shadow,
        crate::style::values::value::Color,
    )],
    inset: bool,
    bw: f32,
    bh: f32,
    dx: f32,
    dy: f32,
    cw: f32,
    ch: f32,
) -> Option<Scene> {
    let outside = Contour::place(outer, bw, bh, dx, dy)?;
    if outside.is_empty() {
        return None;
    }
    let half = if inner.is_some() { 0.0 } else { stroke / 2.0 };
    let inside = match inner {
        Some(inner) => Contour::place(inner, bw, bh, dx, dy)?,
        None => outside.clone(),
    };
    let mut body = Vec::new();
    for (sh, colour) in shadows.iter().rev() {
        if colour.a <= 0.0 {
            continue;
        }
        let total = sh.spread + half;
        let caster = if inset {
            group(vec![
                rectangle(-10000.0, -10000.0, 20000.0, 20000.0, gray(true))?,
                element(inside.offset(false, -total)),
            ])
        } else {
            outside.offset(true, total)
        };
        let mut blurred = group(vec![element(translated(caster, sh.x, sh.y))]);
        blurred.blur = sh.blur.max(0.0) / 2.0;
        let layer = if inset {
            let clip = inside.path(
                Some(gray(true)),
                (half > 0.0).then(|| (gray(false), stroke)),
            );
            let shadow = masked(cw, ch, paint(*colour), blurred)?;
            let mut clipped = group(vec![element(shadow)]);
            clipped.mask = Some(Box::new(clip));
            clipped
        } else {
            let cut = outside.path(
                Some(gray(false)),
                (half > 0.0).then(|| (gray(false), stroke)),
            );
            masked(
                cw,
                ch,
                paint(*colour),
                group(vec![element(blurred), element(cut)]),
            )?
        };
        body.push(element(layer));
    }
    if body.is_empty() {
        return None;
    }
    Some(Scene::new(cw, ch, group(body)))
}
