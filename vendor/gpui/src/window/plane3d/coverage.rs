//! Conservative source ink for projected plane surfaces.

use super::*;

fn finite(b: Bounds<ScaledPixels>) -> bool {
    [b.origin.x.0, b.origin.y.0, b.size.width.0, b.size.height.0]
        .into_iter()
        .all(f32::is_finite)
}

#[cfg(test)]
fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Bounds<ScaledPixels> {
    Bounds::new(
        point(ScaledPixels(x0), ScaledPixels(y0)),
        size(ScaledPixels(x1 - x0), ScaledPixels(y1 - y0)),
    )
}

/// Placed primitive bounds include clipping and nested composites.
/// Retained layers have their own bounds outside primitive_ink.
pub(super) fn source(scene: &Scene, fallback: Bounds<ScaledPixels>) -> Bounds<ScaledPixels> {
    let mut ink = scene.book.primitive_ink;
    if ink.is_some_and(|b| !finite(b)) {
        return fallback;
    }
    for op in &scene.paint_operations {
        if let crate::scene::PaintOperation::Layer(layer) = op {
            if !finite(layer.bounds) {
                return fallback;
            }
            ink = Some(ink.map_or(layer.bounds, |b| b.union(&layer.bounds)));
        }
    }
    ink.filter(|b| finite(*b)).unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_preserves_overflow_and_falls_back_for_invalid_ink() {
        let mut scene = Scene::default();
        let frame = rect(0., 0., 800., 600.);
        scene.book.primitive_ink = Some(rect(-25., 10., 140., 90.));
        assert_eq!(source(&scene, frame), rect(-25., 10., 140., 90.));
        scene.book.primitive_ink = Some(rect(f32::NAN, 10., 140., 90.));
        assert_eq!(source(&scene, frame), frame);
        scene.book.primitive_ink = None;
        assert_eq!(source(&scene, frame), frame);
    }
}
