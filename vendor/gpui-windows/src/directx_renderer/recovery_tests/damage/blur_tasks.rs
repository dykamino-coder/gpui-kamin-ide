//! Reusing a finished blur must preserve every RGBA byte, including mismatch fallback.
use super::*;

fn devices() -> Vec<DirectXDevices> {
    use crate::GpuAdapterPreference as P;
    let mut names = Vec::new();
    let mut out = Vec::new();
    for p in [P::Default, P::HighPerformance, P::Software] {
        let d = DirectXDevices::with_instrumentation(p, gpui::Instrumentation::default()).unwrap();
        let desc = unsafe { d.adapter.GetDesc1() }.unwrap();
        let name = String::from_utf16_lossy(&desc.Description)
            .trim_matches('\0')
            .to_owned();
        if !names.contains(&name) {
            eprintln!("blur task adapter: {name}");
            names.push(name);
            out.push(d);
        }
    }
    out
}

fn scene(changed: bool) -> Scene {
    let mut s = Scene::default();
    for i in 0..12 {
        let x = 8. + (i % 4) as f32 * 60.;
        let y = 8. + (i / 4) as f32 * 54.;
        let bounds = b(x, y, 40., 30.);
        let mut inner = Scene::default();
        let color = if changed && i % 3 == 2 {
            hsla(0.65, 0.9, 0.4, 0.75)
        } else {
            hsla(0.05, 0.8, 0.6, 0.75)
        };
        let mut q = quad([x, y, 40., 30.], color);
        q.order = 1;
        inner.quads.push(q);
        s.groups.push(PaintGroup {
            scene: inner,
            bounds,
            blur_radius: 0.6,
            ..Default::default()
        });
        s.surfaces.push(PaintSurface {
            order: i as u32 + 1,
            bounds,
            content_mask: mask(),
            corner_radii: Corners::default(),
            blur_radius: 0.,
            group: i as u32 + 1,
            opacity: 0.8,
            color_matrix: None,
            fragment: 0,
        });
    }
    s
}

#[test]
fn cached_blur_outputs_are_independent_of_atlas_subresources() {
    let device = DirectXDevices::with_instrumentation(
        crate::GpuAdapterPreference::Software, gpui::Instrumentation::default(),
    ).unwrap();
    let mut renderer = offscreen(&device, W, H);
    renderer.draw(&scene(false), WindowBackgroundAppearance::Opaque).unwrap();
    assert!(renderer.blur.tasks.probes >= 8, "repeated tasks must be exercised");
    assert!(renderer.blur.group_place.windows(2).any(|p| p[0].page == p[1].page),
        "the regression requires multiple groups in one atlas subresource");
    let outputs: Vec<_> = renderer.blur.tasks.outputs().collect();
    assert!(!outputs.is_empty());
    for output in outputs {
        assert!(renderer.blur.groups.iter().all(|page| output != &page.texture),
            "CopySubresourceRegion cannot copy within the same atlas subresource");
    }
}

#[test]
fn validated_blur_tasks_equal_uncached_rgba_for_hits_and_misses() {
    for device in devices() {
        for changed in [false, true] {
            let mut cached = offscreen(&device, W, H);
            let mut uncached = offscreen(&device, W, H);
            uncached.blur.tasks.disabled = true;
            let mut s = scene(changed);
            // Constant nonempty inputs fill the raw rectangle and its halo.
            // Integer scissoring makes translated inputs byte-identical; the
            // changed last group must select the original kernel instead.
            for group in &mut s.groups {
                for q in &mut group.scene.quads {
                    q.bounds.origin.x.0 -= 8.;
                    q.bounds.origin.y.0 -= 8.;
                    q.bounds.size.width.0 += 16.;
                    q.bounds.size.height.0 += 16.;
                    q.corner_radii = Corners::default();
                }
            }
            cached.draw(&s, WindowBackgroundAppearance::Opaque).unwrap();
            uncached
                .draw(&s, WindowBackgroundAppearance::Opaque)
                .unwrap();
            assert!(cached.blur.tasks.probes >= 8, "the GPU predicate must run");
            assert_eq!(uncached.blur.tasks.probes, 0);
            let got = target_bytes(&mut cached);
            assert_eq!(
                cached
                    .blur
                    .tasks
                    .last_difference(&cached.dev().device_context),
                changed
            );
            let want = target_bytes(&mut uncached);
            assert!(
                got == want,
                "changed={changed}: {} differing pixels",
                differing(&got, &want)
            );
        }
    }
}

#[test]
fn mutated_input_rebuilds_the_task_and_retained_frame_without_stale_pixels() {
    for device in devices() {
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
            assert!(
                got == want,
                "frame {i}: {} differing pixels",
                differing(&got, &want)
            );
            previous = s;
        }
    }
}

#[test]
fn removing_all_groups_releases_cached_task_inputs() {
    for device in devices() {
        let mut cached = offscreen(&device, W, H);
        let first = scene(false);
        cached
            .draw(&first, WindowBackgroundAppearance::Opaque)
            .unwrap();
        assert!(cached.blur.tasks.inputs().next().is_some());
        assert!(cached.blur.tasks.outputs().next().is_some());
        let mut empty = Scene::default();
        empty.note_frame_damage(&first, 1, (W, H));
        cached
            .draw(&empty, WindowBackgroundAppearance::Opaque)
            .unwrap();
        assert!(cached.blur.tasks.inputs().next().is_none());
        assert!(cached.blur.tasks.outputs().next().is_none());
    }
}

#[test]
fn a_difference_only_in_the_kernel_halo_rejects_reuse() {
    for device in devices() {
        let mut cached = offscreen(&device, W, H);
        let mut uncached = offscreen(&device, W, H);
        uncached.blur.tasks.disabled = true;
        let mut s = scene(false);
        for group in &mut s.groups {
            group.scene.quads.clear();
        }
        // The last group starts at x=188. Its output begins at 186, but its
        // raw input begins at 184: this extra texel is outside the output.
        let mut halo = quad([185., 126., 1., 8.], hsla(0.6, 1., 0.5, 1.));
        halo.order = 2;
        halo.corner_radii = Corners::default();
        s.groups.last_mut().unwrap().scene.quads.push(halo);
        cached.draw(&s, WindowBackgroundAppearance::Opaque).unwrap();
        uncached
            .draw(&s, WindowBackgroundAppearance::Opaque)
            .unwrap();
        let got = target_bytes(&mut cached);
        assert!(
            cached
                .blur
                .tasks
                .last_difference(&cached.dev().device_context)
        );
        let want = target_bytes(&mut uncached);
        assert!(got == want, "{} differing pixels", differing(&got, &want));
    }
}
