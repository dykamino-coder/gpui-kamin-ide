//! In-place comparison of group marks (`Moved`, kind 7).

use crate::scene::{PaintGroup, PaintSurface, Scene};
use crate::{Background, ContentMask, Hsla, Quad, ScaledPixels, blue, bounds, point, red, size};
use std::rc::Rc;

const W: u32 = 256;
const H: u32 = 256;

fn b(x: f32, y: f32, w: f32, h: f32) -> crate::Bounds<ScaledPixels> {
    bounds(
        point(ScaledPixels(x), ScaledPixels(y)),
        size(ScaledPixels(w), ScaledPixels(h)),
    )
}

fn quad(x: f32, y: f32, w: f32, h: f32, color: Hsla) -> Quad {
    Quad {
        bounds: b(x, y, w, h),
        content_mask: ContentMask {
            bounds: b(0.0, 0.0, W as f32, H as f32),
        },
        background: Background::from(color),
        ..Default::default()
    }
}

/// A box of `color` above a group at (100, 100) whose content is recorded anew.
fn frame(color: Hsla) -> Scene {
    let mut s = Scene::default();
    for (i, q) in [
        quad(0.0, 0.0, W as f32, H as f32, Hsla::white()),
        quad(10.0, 10.0, 100.0, 20.0, color),
    ]
    .into_iter()
    .enumerate()
    {
        s.quads.push(Quad {
            order: i as u32 + 1,
            ..q
        });
    }
    let mut inner = Scene::default();
    inner.quads.push(Quad {
        order: 1,
        ..quad(100.0, 100.0, 50.0, 50.0, red())
    });
    s.groups.push(PaintGroup {
        scene: Rc::new(inner),
        bounds: b(100.0, 100.0, 50.0, 50.0),
        ..Default::default()
    });
    s.surfaces.push(PaintSurface {
        order: 3,
        bounds: b(100.0, 100.0, 50.0, 50.0),
        content_mask: ContentMask {
            bounds: b(0.0, 0.0, W as f32, H as f32),
        },
        corner_radii: Default::default(),
        blur_radius: 0.0,
        group: 1,
        opacity: 0.5,
        color_matrix: None,
        fragment: 0,
    });
    s
}

#[test]
fn a_mark_whose_group_paints_the_same_image_is_equal_in_place() {
    let (old, new) = (frame(red()), frame(blue()));
    let plan = |groups: &[u32]| {
        new.plan_shift_within(&old, (W, H), (0, 0), groups, &mut Default::default(), None)
            .map(|s| s.redraw)
    };
    // Matched with the last frame's group: only the box that changed.
    let matched = plan(&[1]).unwrap();
    assert!(
        !matched.is_empty() && matched.iter().all(|r| r[1] + r[3] <= 40),
        "{matched:?}"
    );
    // Unmatched (its image is not kept): the mark is redrawn too.
    let unmatched = plan(&[0]).unwrap();
    assert!(
        unmatched.iter().any(|r| r[1] + r[3] >= 150),
        "{unmatched:?}"
    );
}
