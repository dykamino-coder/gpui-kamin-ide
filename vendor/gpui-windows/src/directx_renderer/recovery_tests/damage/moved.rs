//! KaminIDE patch: a scroll step drawn as the kept canvas moved, then redrawn
//! within the planned rectangles (`canvas.rs`, `Scene::frame_shift`), holds
//! byte for byte what a whole redraw of the frame draws: a page background
//! that stays, rounded boxes and a shadow that scroll, a bar that stays on
//! top, on both steps down and up.

use super::*;

/// The page at scroll offset `top` (device px): background, boxes, a shadow
/// and a fixed bar over them.
fn page(top: f32) -> Scene {
    let mut s = Scene::default();
    let mut order = 0;
    let mut next = || {
        order += 1;
        order
    };
    let mut background = quad([0., 0., W as f32, H as f32], hsla(0., 0., 1., 1.));
    background.corner_radii = Corners::default();
    background.order = next();
    s.quads.push(background);
    let bounds = b(20., top + 60., 50., 30.);
    s.shadows.push(Shadow {
        order: next(),
        blur_radius: ScaledPixels(5.),
        bounds,
        corner_radii: Corners::all(ScaledPixels(3.)),
        content_mask: mask(),
        color: hsla(0., 0., 0., 0.5),
        element_bounds: bounds,
        element_corner_radii: Corners::all(ScaledPixels(3.)),
        inset: 0,
        pad: 0,
    });
    for k in 0..14 {
        let mut q = quad(
            [12. + 9. * (k % 3) as f32, top + 30. * k as f32, 120., 18.],
            hsla(k as f32 / 14., 0.7, 0.5, 0.9),
        );
        q.order = next();
        s.quads.push(q);
    }
    let mut bar = quad([0., 0., W as f32, 20.], hsla(0.6, 0.5, 0.3, 1.));
    bar.corner_radii = Corners::default();
    bar.order = next();
    s.quads.push(bar);
    s
}

/// Frames at `tops` drawn in sequence, each a move of the last by the step
/// between them; the canvas after the last, and whether each step was a move.
fn scroll(device: &DirectXDevices, tops: &[f32]) -> (Vec<u8>, Vec<bool>) {
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
    (target_bytes(&mut r), moved)
}

#[test]
fn a_frame_compared_in_place_redrawn_apart_equals_the_whole_frame() {
    // Two boxes far apart (rows 30 and 150) change colour; nothing moves.
    let edited = || {
        let mut s = page(0.);
        for i in [2, 6] {
            s.quads[i].background = hsla(0.3, 0.9, 0.4, 1.).into();
        }
        s
    };
    let device = DirectXDevices::new().unwrap();
    let mut r = offscreen(&device, W, H);
    let mut first = page(0.);
    first.note_frame_damage(&Scene::default(), 1, (W, H));
    r.draw(&first, WindowBackgroundAppearance::Opaque).unwrap();
    let mut second = edited();
    second.note_frame_damage_moved(&first, 2, (W, H), Some(Ok((0, 0))));
    let shift = second.frame_shift().cloned().expect("changes far apart");
    assert!(shift.in_place() && shift.redraw.len() >= 2, "{shift:?}");
    r.draw(&second, WindowBackgroundAppearance::Opaque).unwrap();
    let got = target_bytes(&mut r);
    let mut whole = offscreen(&device, W, H);
    let mut fresh = edited();
    fresh.note_frame_damage(&Scene::default(), 1, (W, H));
    whole
        .draw(&fresh, WindowBackgroundAppearance::Opaque)
        .unwrap();
    let want = target_bytes(&mut whole);
    assert!(
        got == want,
        "{} pixels differ from the whole frame",
        differing(&got, &want)
    );
}

#[test]
fn a_scroll_step_drawn_as_a_move_equals_the_whole_frame() {
    let device = DirectXDevices::new().unwrap();
    let tops = [0., -24., -70., -40., -41.];
    for n in 2..=tops.len() {
        let (got, moved) = scroll(&device, &tops[..n]);
        assert!(
            moved[1..].iter().all(|&m| m),
            "steps {:?}: every step is a move ({moved:?})",
            &tops[..n]
        );
        let (want, _) = scroll(&device, &tops[n - 1..n]);
        assert!(
            got == want,
            "after {:?}: {} pixels differ from the whole frame",
            &tops[..n],
            differing(&got, &want)
        );
    }
}
