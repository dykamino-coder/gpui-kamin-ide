//! Обрезка группы внешним контуром `corner-shape` (css-borders-4
//! §corner-shape-rendering: фон и содержимое режет внешний контур). Контур
//! строит `paint::background::contour_outline`; здесь он становится либо
//! многоугольником шейдера (до восьми вершин, как `clip-path: polygon()`),
//! либо маской `pathdef:` (как `clip-path: path()`).

use crate::paint::effects::grouped_element::Grouped;
use gpui::{Bounds, Pixels, Point, point, px};

/// Многоугольник обрезки, замена маски и признак «своя маска группы
/// остаётся». Явный `clip-path: polygon()` не трогается.
pub(super) fn outer(
    group: &Grouped,
    shape_box: Bounds<Pixels>,
    polygon: Vec<Point<Pixels>>,
) -> (Vec<Point<Pixels>>, Option<String>, bool) {
    if !polygon.is_empty() {
        return (polygon, None, true);
    }
    let Some(spec) = group
        .mask
        .as_deref()
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
        _ => (
            Vec::new(),
            Some(if let Some(clip) = &group.corner_clip {
                let [t, _, _, l] = clip.insets;
                let d = crate::paint::decorations::shaped_outset::moved(
                    &path,
                    (-l).max(0.0) + 1.0,
                    (-t).max(0.0) + 1.0,
                );
                format!("pathdef:nonzero:{d}")
            } else {
                format!("pathdef:nonzero:{}", path.svg_d())
            }),
            false,
        ),
    }
}
