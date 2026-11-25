//! Tests of the instances gathered for redraw rectangles (`cull`).

use super::*;
use gpui::{point, size};

fn b(x: f32, y: f32, w: f32, h: f32) -> Bounds<ScaledPixels> {
    Bounds {
        origin: point(ScaledPixels(x), ScaledPixels(y)),
        size: size(ScaledPixels(w), ScaledPixels(h)),
    }
}

fn r(x: u32, y: u32, width: u32, height: u32) -> Region {
    Region {
        x,
        y,
        width,
        height,
    }
}

#[test]
fn every_instance_is_copied_once_per_rectangle_it_reaches_in_batch_order() {
    let mask = ContentMask {
        bounds: b(0.0, 0.0, 800.0, 600.0),
    };
    // Ten rows 60 px apart; a strip at the bottom and a bar on the right.
    let items: Vec<_> = (0..10)
        .map(|k| (b(0.0, 60.0 * k as f32, 800.0, 20.0), mask))
        .collect();
    let rects = disjoint(&[r(0, 480, 800, 120), r(780, 0, 20, 600)]);
    let mut out = Vec::new();
    let (n, _) = gather(
        &mut out,
        &items,
        &rects,
        |i| extent(i.0, &i.1, None, 1.0),
        |i| &mut i.1,
    );
    assert_eq!(n, out.len());
    // Rows 0..8 reach only the bar above the strip, row 9 only the strip,
    // row 8 both (its antialiasing margin crosses the strip's top edge).
    let ys: Vec<f32> = out.iter().map(|i| i.0.origin.y.0).collect();
    let want: Vec<f32> = (0..8)
        .map(|k| 60.0 * k as f32)
        .chain([480.0, 480.0, 540.0])
        .collect();
    assert_eq!(ys, want);
    for (i, item) in out.iter().enumerate() {
        let m = item.1.bounds;
        let inside = rects.iter().any(|q| {
            m.origin.x.0 >= q[0]
                && m.origin.y.0 >= q[1]
                && m.origin.x.0 + m.size.width.0 <= q[2]
                && m.origin.y.0 + m.size.height.0 <= q[3]
        });
        assert!(inside, "copy {i} {m:?} outside every rectangle");
    }
}

#[test]
fn an_edge_the_rectangle_keeps_keeps_its_float() {
    // A mask edge at a pixel centre: the shader keeps pixels at distance 0.
    let mut m = ContentMask {
        bounds: b(10.5, 3.25, 489.5, 100.0),
    };
    let end = m.bounds.origin.x.0 + m.bounds.size.width.0;
    assert!(cut(&mut m, &[100.0, 0.0, 800.0, 600.0]));
    assert_eq!(m.bounds.origin.x.0, 100.0);
    assert_eq!(m.bounds.origin.x.0 + m.bounds.size.width.0, end);
    assert_eq!((m.bounds.origin.y.0, m.bounds.size.height.0), (3.25, 100.0));
    let mut whole = ContentMask {
        bounds: b(1.0 / 3.0, 0.1, 7.7, 9.9),
    };
    let before = whole;
    assert!(cut(&mut whole, &[0.0, 0.0, 800.0, 600.0]));
    assert_eq!(whole, before);
    let mut outside = ContentMask {
        bounds: b(0.0, 0.0, 10.0, 10.0),
    };
    assert!(!cut(&mut outside, &[10.0, 0.0, 20.0, 10.0]));
}

#[test]
fn rectangles_become_disjoint_and_cover_the_same_pixels() {
    let rects = [r(0, 0, 100, 100), r(50, 50, 100, 100), r(0, 0, 10, 10)];
    let parts = disjoint(&rects);
    let covered = |x: f32, y: f32| {
        parts
            .iter()
            .filter(|p| p[0] <= x && x < p[2] && p[1] <= y && y < p[3])
            .count()
    };
    for y in (0..160).step_by(5) {
        for x in (0..160).step_by(5) {
            let (x, y) = (x as f32 + 0.5, y as f32 + 0.5);
            let want = usize::from(
                x < 100.0 && y < 100.0 || (50.0..150.0).contains(&x) && (50.0..150.0).contains(&y),
            );
            assert_eq!(covered(x, y), want, "pixel {x} {y}");
        }
    }
    assert_eq!(bounds(&parts), Some(r(0, 0, 150, 150)));
}

#[test]
fn a_transformation_reaches_every_rectangle() {
    let clipped = ContentMask {
        bounds: b(0.0, 0.0, 800.0, 100.0),
    };
    assert_eq!(
        extent(b(0.0, 50.0, 10.0, 500.0), &clipped, None, 1.0).map(|e| e[3]),
        Some(100.0)
    );
    let mut turned = TransformationMatrix::unit();
    turned.translation[1] = 500.0;
    let e = extent(b(0.0, 0.0, 10.0, 10.0), &clipped, Some(&turned), 1.0);
    assert_eq!(e, None);
    assert!(meets(e, &[700.0, 500.0, 800.0, 600.0]));
}
