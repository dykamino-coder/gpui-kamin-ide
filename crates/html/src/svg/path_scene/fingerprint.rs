//! Content identity for cached path scenes, including masks and filter geometry.

use super::*;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub(super) fn scene(scene: &Scene) -> u64 {
    let mut h = DefaultHasher::new();
    // Keep the typed-scene domain separate from markup raster keys.
    "path-scene".hash(&mut h);
    scene.width.to_bits().hash(&mut h);
    scene.height.to_bits().hash(&mut h);
    hash_group(&scene.root, &mut h);
    h.finish()
}

fn hash_paint(p: Option<PathPaint>, h: &mut impl Hasher) {
    p.is_some().hash(h);
    if let Some(p) = p {
        [p.color.red, p.color.green, p.color.blue].hash(h);
        p.opacity.get().to_bits().hash(h);
    }
}

fn hash_group(g: &PathGroup, h: &mut impl Hasher) {
    let t = g.transform;
    for v in [t.sx, t.kx, t.ky, t.sy, t.tx, t.ty, g.blur] {
        v.to_bits().hash(h);
    }
    g.children.len().hash(h);
    for c in &g.children {
        match c {
            PathElement::Group(c) => {
                0u8.hash(h);
                hash_group(c, h);
            }
            PathElement::Shape(p) => {
                hash_shape(p, h);
            }
        }
    }
    g.clip.is_some().hash(h);
    if let Some(clip) = &g.clip {
        hash_shape(clip, h);
    }
    g.mask.is_some().hash(h);
    if let Some(mask) = &g.mask {
        hash_group(mask, h);
    }
}

fn hash_shape(p: &PathShape, h: &mut impl Hasher) {
    1u8.hash(h);
    (p.rule == FillRule::EvenOdd).hash(h);
    hash_paint(p.fill, h);
    p.stroke.is_some().hash(h);
    if let Some(s) = p.stroke {
        hash_paint(Some(s.paint), h);
        s.width.get().to_bits().hash(h);
        s.miter_limit.get().to_bits().hash(h);
    }
    for s in p.data.segments() {
        use gpui::path_scene::tiny_skia_path::PathSegment::*;
        let (tag, points, count) = match s {
            MoveTo(p) => (0, [p, p, p], 1),
            LineTo(p) => (1, [p, p, p], 1),
            QuadTo(a, b) => (2, [a, b, b], 2),
            CubicTo(a, b, c) => (3, [a, b, c], 3),
            Close => (
                4,
                [gpui::path_scene::tiny_skia_path::Point::from_xy(0.0, 0.0); 3],
                0,
            ),
        };
        tag.hash(h);
        for p in points.iter().take(count) {
            p.x.to_bits().hash(h);
            p.y.to_bits().hash(h);
        }
    }
    5u8.hash(h);
}
