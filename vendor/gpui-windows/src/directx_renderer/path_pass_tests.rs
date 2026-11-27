//! KaminIDE patch: the area a path batch works in (`path_area`).

use super::{Region, path_area, scissor_bounds};
use gpui::{Bounds, ContentMask, Path, Pixels, ScaledPixels, point, px, size};
use windows::Win32::Foundation::RECT;

/// A triangle spanning `x0..x1` by `y0..y1`, its content mask `mask`.
fn path(x0: f32, y0: f32, x1: f32, y1: f32, mask: [f32; 4]) -> Path<ScaledPixels> {
    let mut p = Path::<Pixels>::new(point(px(x0), px(y0)));
    p.line_to(point(px(x1), px(y0)));
    p.line_to(point(px(x0), px(y1)));
    p.content_mask = ContentMask {
        bounds: Bounds {
            origin: point(px(mask[0]), px(mask[1])),
            size: size(px(mask[2]), px(mask[3])),
        },
    };
    p.scale(1.)
}

const OPEN: [f32; 4] = [0., 0., 1000., 1000.];

fn region(x: u32, y: u32, width: u32, height: u32) -> Region {
    Region {
        x,
        y,
        width,
        height,
    }
}

#[test]
fn the_area_is_the_whole_pixels_of_the_clipped_bounds_of_every_path() {
    let paths = [
        path(10.5, 20.25, 30.5, 40., OPEN),
        path(50., 5.75, 60.2, 9., OPEN),
    ];
    assert_eq!(
        path_area(&paths, (200, 100), None),
        Some(region(10, 5, 51, 35))
    );
    // The content mask clips the bounds before rounding.
    let masked = [path(10., 10., 90., 90., [20., 30., 10.5, 5.])];
    assert_eq!(
        path_area(&masked, (200, 100), None),
        Some(region(20, 30, 11, 5))
    );
}

#[test]
fn the_target_and_the_scissor_cut_the_area() {
    let paths = [path(-20., -10., 300., 50., OPEN)];
    assert_eq!(
        path_area(&paths, (200, 100), None),
        Some(region(0, 0, 200, 50))
    );
    let scissor = Some(Some(region(150, 40, 32, 32)));
    assert_eq!(
        path_area(&paths, (200, 100), scissor),
        Some(region(150, 40, 32, 10))
    );
}

#[test]
fn a_batch_that_reaches_nothing_has_no_area() {
    let paths = [path(10., 10., 40., 40., OPEN)];
    // A redraw rectangle elsewhere.
    assert_eq!(
        path_area(&paths, (200, 100), Some(Some(region(100, 60, 32, 32)))),
        None
    );
    // The scissor on with no rectangle passes nothing.
    assert_eq!(path_area(&paths, (200, 100), Some(None)), None);
    // Outside the target, and no paths at all.
    assert_eq!(
        path_area(&[path(300., 10., 320., 40., OPEN)], (200, 100), None),
        None
    );
    assert_eq!(path_area(&[], (200, 100), None), None);
}

#[test]
fn bound_scissor_rectangles_unite_and_empty_ones_count_for_nothing() {
    let r = |left, top, right, bottom| RECT {
        left,
        top,
        right,
        bottom,
    };
    assert_eq!(
        scissor_bounds(&[r(10, 20, 30, 40), r(5, 30, 12, 60), r(50, 50, 50, 90)]),
        Some(region(5, 20, 25, 40))
    );
    assert_eq!(scissor_bounds(&[r(-5, -5, 8, 4)]), Some(region(0, 0, 8, 4)));
    assert_eq!(scissor_bounds(&[r(4, 4, 4, 9)]), None);
    assert_eq!(scissor_bounds(&[]), None);
}
