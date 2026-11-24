//! KaminIDE patch: groups that scroll with the page (`Scene::group_match`
//! with a step) keep their images (`group_cache`): a step draws again only
//! the groups whose region the window edge cut, and the frame equals its
//! whole redraw byte for byte.

use super::*;

/// The 4×3 grid of groups over a page background, scrolled by `top`.
fn page(top: f32) -> Scene {
    let mut s = Scene::default();
    let mut background = quad([0., 0., W as f32, H as f32], hsla(0., 0., 1., 1.));
    background.corner_radii = Corners::default();
    background.order = 1;
    s.quads.push(background);
    for cell in 0..12 {
        let (x, y) = ((cell % 4) as f32 * 60. + 8., (cell / 4) as f32 * 60. + 8. + top);
        let mut inner = Scene::default();
        let mut q = quad([x, y, 44., 44.], color(cell));
        q.order = 1;
        inner.quads.push(q);
        let bounds = b(x, y, 44., 44.);
        s.groups.push(PaintGroup { scene: std::rc::Rc::new(inner), bounds, ..Default::default() });
        s.surfaces.push(PaintSurface {
            order: cell as u32 + 2,
            bounds,
            content_mask: mask(),
            corner_radii: Corners::default(),
            blur_radius: 0.,
            group: cell as u32 + 1,
            opacity: 0.8,
            color_matrix: None,
            fragment: 0,
        });
    }
    s
}

/// Frames at `tops` in sequence, each offered the step from the last: the
/// target, whether each step was a move, the group counts of the last frame.
fn scroll(device: &DirectXDevices, tops: &[f32]) -> (Vec<u8>, Vec<bool>, GroupCounts) {
    let mut r = offscreen(device, W, H);
    let mut previous = Scene::default();
    let mut moved = Vec::new();
    for (i, &top) in tops.iter().enumerate() {
        let mut s = page(top);
        let step = (i > 0).then(|| Ok((0, (top - tops[i - 1]) as i32)));
        s.note_frame_damage_moved(&previous, i as u64 + 1, (W, H), step);
        moved.push(s.frame_shift().is_some());
        r.draw(&s, WindowBackgroundAppearance::Opaque).unwrap();
        previous = s;
    }
    (target_bytes(&mut r), moved, r.retained.groups.counts)
}

#[test]
fn groups_that_scroll_keep_their_images_and_the_frame_equals_its_whole_draw() {
    let device = DirectXDevices::new().unwrap();
    let tops = [0., -12., -30., -20.];
    for n in 2..=tops.len() {
        let (got, moved, counts) = scroll(&device, &tops[..n]);
        assert!(moved[1..].iter().all(|&m| m), "steps {:?}: {moved:?}", &tops[..n]);
        // The top row meets the window edge; the two rows below keep their images.
        assert!(counts.kept >= 8, "steps {:?}: {counts:?}", &tops[..n]);
        let (want, _, _) = scroll(&device, &tops[n - 1..n]);
        assert!(got == want, "after {:?}: {} pixels differ from the whole frame", &tops[..n], differing(&got, &want));
    }
}