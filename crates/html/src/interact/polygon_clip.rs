//! Resolve polygon coordinates and route axis-aligned rectangles through a device clip.
use super::{Grouped, legacy_clip};
use crate::value::Len;
use gpui::{Bounds, Pixels, Point, point, px, size};

pub(super) fn points(group: &Grouped, bounds: Bounds<Pixels>) -> Vec<Point<Pixels>> {
    let [top, right, bottom, left] = group.poly_expand;
    let base = Bounds {
        origin: point(bounds.origin.x - px(left), bounds.origin.y - px(top)),
        size: size(
            bounds.size.width + px(left + right),
            bounds.size.height + px(top + bottom),
        ),
    };
    let coord = |value: Len, side: Pixels| match value {
        Len::Pct(p) => side * p,
        Len::Px(v) => px(v),
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

pub(super) fn rectangle(points: &[Point<Pixels>], scale: f32) -> Option<[f32; 4]> {
    if points.len() != 4 {
        return None;
    }
    let mut horizontal = [false; 4];
    for i in 0..4 {
        let a = points[i];
        let b = points[(i + 1) % 4];
        horizontal[i] = a.y == b.y;
        if !f32::from(a.x).is_finite()
            || !f32::from(a.y).is_finite()
            || horizontal[i] == (a.x == b.x)
        {
            return None;
        }
    }
    if (0..4).any(|i| horizontal[i] == horizontal[(i + 1) % 4]) {
        return None;
    }
    let left = points
        .iter()
        .map(|p| f32::from(p.x))
        .fold(f32::INFINITY, f32::min);
    let top = points
        .iter()
        .map(|p| f32::from(p.y))
        .fold(f32::INFINITY, f32::min);
    let right = points
        .iter()
        .map(|p| f32::from(p.x))
        .fold(f32::NEG_INFINITY, f32::max);
    let bottom = points
        .iter()
        .map(|p| f32::from(p.y))
        .fold(f32::NEG_INFINITY, f32::max);
    // CSS Shapes §3.1 polygon() connects consecutive vertices with straight
    // segments. Four alternating axis-aligned segments enclose the same region
    // as inset(); use that clip's final device edges rather than polygon SDF AA.
    Some(legacy_clip::snap([
        left * scale,
        top * scale,
        (right - left) * scale,
        (bottom - top) * scale,
    ]))
}

pub(super) fn geometry(
    group: &Grouped,
    bounds: Bounds<Pixels>,
    reference: Bounds<Pixels>,
    scale: f32,
) -> (Vec<Point<Pixels>>, Option<[f32; 4]>) {
    if let Some(rect) = rectangle(&points(group, reference), scale) {
        (Vec::new(), Some(rect))
    } else {
        (points(group, bounds), None)
    }
}

pub(super) fn intersect(a: Option<[f32; 4]>, b: Option<[f32; 4]>) -> Option<[f32; 4]> {
    let (Some([ax, ay, aw, ah]), Some([bx, by, bw, bh])) = (a, b) else {
        return a.or(b);
    };
    // A mask painting area and a clip-path both constrain the same group.
    let (left, top) = (ax.max(bx), ay.max(by));
    let (right, bottom) = ((ax + aw).min(bx + bw), (ay + ah).min(by + bh));
    Some(if right <= left || bottom <= top {
        legacy_clip::snap([left, top, 0.0, 0.0])
    } else {
        [left, top, right - left, bottom - top]
    })
}
