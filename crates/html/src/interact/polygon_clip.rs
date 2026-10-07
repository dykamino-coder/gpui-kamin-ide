//! Share rectangular polygon geometry with the rectangular clip raster path.
use super::Grouped;
use crate::value::Len;
use gpui::{Bounds, LayoutId, Pixels, Point, Window, point, px, size};

pub(super) fn points(group: &Grouped, bounds: Bounds<Pixels>) -> Vec<Point<Pixels>> {
    let [top, right, bottom, left] = group.poly_expand;
    let base = Bounds {
        origin: point(bounds.origin.x - px(left), bounds.origin.y - px(top)),
        size: size(
            bounds.size.width + px(left + right),
            bounds.size.height + px(top + bottom),
        ),
    };
    let coord = |len: Len, side: Pixels| match len {
        Len::Pct(value) => side * value,
        Len::Px(value) => px(value),
        _ => px(0.0),
    };
    group
        .polygon
        .iter()
        .map(|(x, y)| {
            point(
                base.origin.x + coord(*x, base.size.width),
                base.origin.y + coord(*y, base.size.height),
            )
        })
        .collect()
}

pub(super) fn rectangle(group: &Grouped, bounds: Bounds<Pixels>) -> Option<[f32; 4]> {
    if group.polygon.len() != 4
        || group.mask_clip_off.is_some()
        || group.clip_rect.is_some()
        || group.clip_inset.is_some()
        || group.clip_edges.is_some()
        || group.clip_xywh.is_some()
        || group.polygon.iter().any(|(x, y)| {
            !matches!(x, Len::Px(_) | Len::Pct(_)) || !matches!(y, Len::Px(_) | Len::Pct(_))
        })
    {
        return None;
    }
    let points = points(group, bounds);
    let mut edges = [
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    ];
    for (index, p) in points.iter().enumerate() {
        let (x, y) = (f32::from(p.x), f32::from(p.y));
        let next = points[(index + 1) % 4];
        if !x.is_finite() || !y.is_finite() || (p.x == next.x) == (p.y == next.y) {
            return None;
        }
        edges = [
            edges[0].min(x),
            edges[1].min(y),
            edges[2].max(x),
            edges[3].max(y),
        ];
    }
    let [left, top, right, bottom] = edges;
    let mut corners = 0;
    for p in &points {
        let (x, y) = (f32::from(p.x), f32::from(p.y));
        if (x != left && x != right) || (y != top && y != bottom) {
            return None;
        }
        corners |= 1 << (u32::from(x == right) + 2 * u32::from(y == bottom));
    }
    // CSS Shapes 1 §3.1: four distinct corners joined by axis-aligned edges
    // enclose the same region as a rectangle, under either fill rule.
    (corners == 15).then_some([left, top, right - left, bottom - top])
}

pub(super) fn logical_rectangle(
    group: &Grouped,
    id: LayoutId,
    window: &mut Window,
) -> Option<[f32; 4]> {
    if group.polygon.len() != 4 {
        return None;
    }
    // Test the logical shape during prepaint: rounded box dimensions can
    // make a trapezoid rectangular when percentages mix with absolute lengths.
    let bounds = Bounds {
        origin: window.layout_origin_unrounded(id),
        size: window.layout_size_unrounded(id),
    };
    rectangle(group, bounds)
}

pub(super) fn device_clip(rectangle: Option<[f32; 4]>, window: &Window) -> Option<[f32; 4]> {
    if window.current_transformation() != gpui::TransformationMatrix::unit() {
        return None;
    }
    let sf = window.scale_factor();
    rectangle.map(|rect| super::rectangular_clip::snap_edges(rect.map(|v| v * sf)))
}
