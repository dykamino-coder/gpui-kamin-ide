//! KaminIDE patch: groups that move with a scroll step are matched moved and
//! their composites compare equal to the moved ones (`shift::groups`).

use crate::scene::{PaintGroup, PaintSurface, Scene};
use crate::{Background, ContentMask, Corners, Quad, ScaledPixels, bounds, hsla, point, size};
use std::rc::Rc;

const W: u32 = 256;
const H: u32 = 256;

fn rect(x: f32, y: f32, w: f32, h: f32) -> crate::Bounds<ScaledPixels> {
    bounds(
        point(ScaledPixels(x), ScaledPixels(y)),
        size(ScaledPixels(w), ScaledPixels(h)),
    )
}

/// A page background and one opacity group per row, every row moved by `top`;
/// group `edited` (if any) paints another colour.
fn page(top: f32, edited: Option<usize>) -> Scene {
    let window = ContentMask {
        bounds: rect(0., 0., W as f32, H as f32),
    };
    let mut s = Scene::default();
    s.quads.push(Quad {
        order: 1,
        bounds: rect(0., 0., W as f32, H as f32),
        content_mask: window.clone(),
        background: Background::from(hsla(0., 0., 1., 1.)),
        ..Default::default()
    });
    for k in 0..6 {
        let at = rect(16., top + 40. * k as f32, 200., 24.);
        let hue = if edited == Some(k) {
            0.5
        } else {
            k as f32 / 6.
        };
        let mut inner = Scene::default();
        inner.quads.push(Quad {
            order: 1,
            bounds: at,
            content_mask: window.clone(),
            corner_radii: Corners::all(ScaledPixels(4.)),
            background: Background::from(hsla(hue, 0.6, 0.5, 1.)),
            ..Default::default()
        });
        s.groups.push(PaintGroup {
            scene: Rc::new(inner),
            bounds: at,
            ..Default::default()
        });
        s.surfaces.push(PaintSurface {
            order: k as u32 + 2,
            bounds: at,
            content_mask: window.clone(),
            corner_radii: Corners::default(),
            blur_radius: 0.,
            group: k as u32 + 1,
            opacity: 0.7,
            color_matrix: None,
            fragment: 0,
        });
    }
    s
}

fn step(old: Scene, mut new: Scene, d: (i32, i32)) -> Scene {
    let mut old = old;
    old.note_frame_damage(&Scene::default(), 1, (W, H));
    new.note_frame_damage_moved(&old, 2, (W, H), Some(Ok(d)));
    new
}

#[test]
fn groups_that_scroll_are_matched_moved_and_redraw_only_the_strip() {
    let new = step(page(0., None), page(-20., None), (0, -20));
    for g in 0..6 {
        assert_eq!(new.group_match(g), Some((g, (0, -20))), "group {g}");
    }
    let shift = new.frame_shift().cloned().expect("the step is a move");
    assert_eq!(shift.redraw, vec![[0, H - 20, W, 20]], "{shift:?}");
}

#[test]
fn a_group_changed_during_the_step_is_redrawn_and_not_matched() {
    let new = step(page(0., None), page(-20., Some(2)), (0, -20));
    assert_eq!(new.group_match(2), None);
    assert_eq!(new.group_match(3), Some((3, (0, -20))));
    let shift = new.frame_shift().cloned().expect("the step is a move");
    // Row 2 lands at y 60..84: a redraw covers it.
    assert!(
        shift.redraw.iter().any(|r| r[1] <= 60 && r[1] + r[3] >= 84),
        "{shift:?}"
    );
}
