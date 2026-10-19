//! Обрезка группы внешним контуром `corner-shape` (css-borders-4
//! §corner-shape-rendering: фон и содержимое режет внешний контур). Контур
//! строит `paint::background::contour_outline`; здесь он становится либо
//! многоугольником шейдера (до восьми вершин, как `clip-path: polygon()`),
//! либо растром покрытия контура tiny-skia.

use crate::paint::effects::grouped_element::Grouped;
use crate::svg::path_commands::Geometry;
use crate::svg::path_scene::{FillRule, Scene, coverage};
use gpui::{Bounds, Pixels, Point, point, px};

/// Многоугольник обрезки, замена маски и признак «своя маска группы
/// остаётся». Явный `clip-path: polygon()` не трогается.
pub(super) fn outer(
    group: &Grouped,
    shape_box: Bounds<Pixels>,
    polygon: Vec<Point<Pixels>>,
) -> (Vec<Point<Pixels>>, Option<Scene>, bool) {
    if !polygon.is_empty() {
        return (polygon, None, true);
    }
    let Some(spec) = group
        .mask
        .as_ref()
        .and_then(|m| m.geometry())
        .and_then(|m| m.strip_prefix("shape:rrect("))
        .and_then(|s| s.strip_suffix(')'))
    else {
        return (polygon, None, true);
    };
    let (bw, bh) = (
        f32::from(shape_box.size.width),
        f32::from(shape_box.size.height),
    );
    let path = match group.corner_clip.as_ref() {
        Some(clip) => crate::paint::background::contour_offset(spec, bw, bh, clip.insets),
        None => crate::paint::background::contour_outline(spec, bw, bh),
    };
    let Some(path) = path else {
        return (polygon, None, true);
    };
    match path.polygon() {
        Some(p) if (3..=8).contains(&p.len()) => {
            let o = shape_box.origin;
            let pts = p
                .iter()
                .map(|&(x, y)| point(o.x + px(x), o.y + px(y)))
                .collect();
            (pts, None, false)
        }
        _ => {
            let mut d = Geometry::default();
            let (dx, dy) = group.corner_clip.as_ref().map_or((0.0, 0.0), |clip| {
                let [t, _, _, l] = clip.insets;
                ((-l).max(0.0) + 1.0, (-t).max(0.0) + 1.0)
            });
            path.commands(&mut d, dx, dy);
            let mask_box = crate::paint::effects::corner_clip::mask_box(group, shape_box);
            let scene = Some(coverage(
                f32::from(mask_box.size.width),
                f32::from(mask_box.size.height),
                d.finish(),
                FillRule::NonZero,
            ));
            (Vec::new(), scene, false)
        }
    }
}
