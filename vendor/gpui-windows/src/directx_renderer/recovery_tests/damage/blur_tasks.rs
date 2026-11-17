//! Reusing a finished blur must preserve every RGBA byte, including mismatch fallback.
use super::*;

fn scene(changed: bool) -> Scene {
    let mut s = Scene::default();
    for i in 0..12 {
        let x = 8. + (i % 4) as f32 * 60.;
        let y = 8. + (i / 4) as f32 * 54.;
        let bounds = b(x, y, 40., 30.);
        let mut inner = Scene::default();
        let color = if changed && i % 3 == 2 {
            hsla(0.65, 0.9, 0.4, 0.75)
        } else { hsla(0.05, 0.8, 0.6, 0.75) };
        let mut q = quad([x, y, 40., 30.], color);
        q.order = 1;
        inner.quads.push(q);
        s.groups.push(PaintGroup { scene: inner, bounds, blur_radius: 0.6, ..Default::default() });
        s.surfaces.push(PaintSurface {
            order: i as u32 + 1, bounds, content_mask: mask(),
            corner_radii: Corners::default(), blur_radius: 0.,
            group: i as u32 + 1, opacity: 0.8, color_matrix: None, fragment: 0,
        });
    }
    s
}

#[test]
fn validated_blur_tasks_equal_uncached_rgba_for_hits_and_misses() {
    let device = DirectXDevices::new().unwrap();
    for changed in [false, true] {
        let mut cached = offscreen(&device, W, H);
        let mut uncached = offscreen(&device, W, H);
        uncached.blur.tasks.disabled = true;
        let s = scene(changed);
        cached.draw(&s, WindowBackgroundAppearance::Opaque).unwrap();
        uncached.draw(&s, WindowBackgroundAppearance::Opaque).unwrap();
        assert!(cached.blur.tasks.probes >= 8, "the GPU predicate must run");
        assert_eq!(uncached.blur.tasks.probes, 0);
        let got = target_bytes(&mut cached);
        assert_eq!(cached.blur.tasks.last_difference(&cached.dev().device_context), changed);
        let want = target_bytes(&mut uncached);
        assert!(got == want, "changed={changed}: {} differing pixels", differing(&got, &want));
    }
}

#[test]
fn mutated_input_rebuilds_the_task_and_retained_frame_without_stale_pixels() {
    let device = DirectXDevices::new().unwrap();
    let mut cached = offscreen(&device, W, H);
    let mut previous = Scene::default();
    for (i, changed) in [false, true, false].into_iter().enumerate() {
        let mut s = scene(changed);
        s.note_frame_damage(&previous, i as u64 + 1, (W, H));
        cached.draw(&s, WindowBackgroundAppearance::Opaque).unwrap();
        let mut whole = offscreen(&device, W, H);
        whole.blur.tasks.disabled = true;
        whole.draw(&s, WindowBackgroundAppearance::Opaque).unwrap();
        let got = target_bytes(&mut cached);
        let want = target_bytes(&mut whole);
        assert!(got == want, "frame {i}: {} differing pixels", differing(&got, &want));
        previous = s;
    }
}

#[test]
fn removing_all_groups_releases_cached_task_inputs() {
    let device = DirectXDevices::new().unwrap();
    let mut cached = offscreen(&device, W, H);
    let first = scene(false);
    cached.draw(&first, WindowBackgroundAppearance::Opaque).unwrap();
    assert!(cached.blur.tasks.inputs().next().is_some());
    let mut empty = Scene::default();
    empty.note_frame_damage(&first, 1, (W, H));
    cached.draw(&empty, WindowBackgroundAppearance::Opaque).unwrap();
    assert!(cached.blur.tasks.inputs().next().is_none());
}
