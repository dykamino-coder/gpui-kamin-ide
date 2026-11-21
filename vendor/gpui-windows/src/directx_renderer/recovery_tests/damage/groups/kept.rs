//! KaminIDE patch: the group cache survives frames that draw nothing and
//! frames that draw only the groups their damage needs (effects-480: 480
//! overlapping filtered groups, one edited, every edit used to draw all
//! needed groups again).

use super::super::super::super::group_cache::GroupCounts;
use super::*;

/// One group per cell of the 4×3 grid; group `k` is `44 - side[k]` px square,
/// so freed rectangles do not fit every other group.
fn sized(cells: &[(usize, Hsla)], side: impl Fn(usize) -> f32) -> Scene {
    let mut s = groups(cells);
    for (k, g) in s.groups.iter_mut().enumerate() {
        let d = side(k);
        let (x, y) = (g.bounds.origin.x.0, g.bounds.origin.y.0);
        g.bounds = b(x, y, 44. - d, 44. - d);
        g.scene.quads[0].bounds = g.bounds;
        s.surfaces[k].bounds = g.bounds;
    }
    s
}

fn whole(device: &DirectXDevices, scene: impl Fn() -> Scene) -> Vec<u8> {
    let mut r = offscreen(device, W, H);
    let mut alone = scene();
    alone.note_frame_damage(&Scene::default(), 1, (W, H));
    r.draw(&alone, WindowBackgroundAppearance::Opaque).unwrap();
    target_bytes(&mut r)
}

/// A frame kept without drawing under a new scene number hands its images
/// to the next frame compared with it.
#[test]
fn a_kept_frame_with_a_new_scene_keeps_the_group_images() {
    let device = DirectXDevices::new().unwrap();
    let base: Vec<(usize, Hsla)> = (0..12).map(|i| (i, color(i))).collect();
    let mut recolored = base.clone();
    recolored[6].1 = hsla(0.95, 0.9, 0.4, 1.);
    let mut r = offscreen(&device, W, H);
    let mut previous = Scene::default();
    for (i, cells) in [&base, &base, &recolored].into_iter().enumerate() {
        let mut s = groups(cells);
        s.note_frame_damage(&previous, i as u64 + 1, (W, H));
        r.draw(&s, WindowBackgroundAppearance::Opaque).unwrap();
        previous = s;
    }
    assert_eq!(r.retained.groups.counts, GroupCounts { kept: 11, drawn: 1 });
    let got = target_bytes(&mut r);
    let want = whole(&device, || groups(&recolored));
    assert!(got == want, "{} pixels differ from the whole frame", differing(&got, &want));
}

/// Groups a frame within a rectangle did not draw keep their rectangles:
/// they take no new space, so the atlas is not packed again and the images
/// that are there stay.
#[test]
fn groups_without_an_image_keep_their_atlas_rectangles() {
    let device = DirectXDevices::new().unwrap();
    let side = |k: usize| (k % 4) as f32 * 3.;
    let base: Vec<(usize, Hsla)> = (0..12).map(|i| (i, color(i))).collect();
    let mut recolored = base.clone();
    recolored[0].1 = hsla(0.95, 0.9, 0.4, 1.);
    let mut r = offscreen(&device, W, H);
    let mut first = sized(&base, side);
    first.note_frame_damage(&Scene::default(), 1, (W, H));
    r.draw(&first, WindowBackgroundAppearance::Opaque).unwrap();
    // As after a frame that drew only the groups of its damage: three images.
    for (g, v) in r.retained.groups.valid.iter_mut().enumerate() {
        *v = matches!(g, 1 | 5 | 9);
    }
    let mut next = sized(&recolored, side);
    next.note_frame_damage(&first, 2, (W, H));
    r.draw(&next, WindowBackgroundAppearance::Opaque).unwrap();
    assert_eq!(r.retained.groups.counts, GroupCounts { kept: 3, drawn: 1 });
    let got = target_bytes(&mut r);
    let want = whole(&device, || sized(&recolored, side));
    assert!(got == want, "{} pixels differ from the whole frame", differing(&got, &want));
}
