//! Navigation releases effect owners; still-used masks survive every frame.

use super::*;
use gpui::{
    Background, Bounds, ContentMask, Corners, PaintGroup, PaintSurface, Quad, ScaledPixels,
    bgra_bytes_to_image, hsla, point, size,
};

const W: u32 = 128;
const H: u32 = 96;

fn bounds(x: f32, y: f32, w: f32, h: f32) -> Bounds<ScaledPixels> {
    Bounds::new(
        point(ScaledPixels(x), ScaledPixels(y)),
        size(ScaledPixels(w), ScaledPixels(h)),
    )
}

fn scene(count: usize, blur: f32, blend: u32, backdrop: bool) -> Scene {
    let mut scene = Scene::default();
    let frame = ContentMask {
        bounds: bounds(0., 0., W as f32, H as f32),
    };
    for i in 0..count {
        let b = bounds(
            (i % 12) as f32 * 10. + 2.,
            (i / 12) as f32 * 10. + 2.,
            8.,
            8.,
        );
        let mut child = Scene::default();
        child.quads.push(Quad {
            bounds: b,
            content_mask: frame,
            background: Background::from(hsla(i as f32 / count as f32, 0.7, 0.4, 0.8)),
            ..Default::default()
        });
        let mut pixels = vec![255; 8 * 8 * 4];
        for (p, pixel) in pixels.chunks_exact_mut(4).enumerate() {
            if p % 3 == 0 {
                pixel.fill(127);
            }
        }
        scene.groups.push(PaintGroup {
            scene: std::rc::Rc::new(child),
            bounds: b,
            blur_radius: blur,
            blend,
            mask: Some(bgra_bytes_to_image(8, 8, pixels).unwrap()),
            mask_bounds: b,
            ..Default::default()
        });
        scene.surfaces.push(PaintSurface {
            order: i as u32 + 1,
            bounds: b,
            content_mask: frame,
            corner_radii: Corners::default(),
            blur_radius: 0.,
            group: i as u32 + 1,
            opacity: 0.7,
            color_matrix: None,
            fragment: 0,
        });
    }
    if backdrop {
        scene.surfaces.push(PaintSurface {
            order: count as u32 + 1,
            bounds: bounds(0., 0., W as f32, H as f32),
            content_mask: frame,
            corner_radii: Corners::default(),
            blur_radius: 5.,
            group: 0,
            opacity: 0.8,
            color_matrix: None,
            fragment: 0,
        });
    }
    scene
}

fn effect_bytes(renderer: &DirectXRenderer) -> u64 {
    let mut bytes = 0;
    renderer.gpu_memory(&mut |owner, allocated, _| {
        if owner.starts_with("gpu/group/") || owner.starts_with("gpu/blur/") {
            bytes += allocated;
        }
    });
    bytes
}

fn take(renderer: &DirectXRenderer) -> String {
    renderer
        .gpu_timer
        .instrumentation
        .batches
        .take_line()
        .unwrap_or_default()
}

#[test]
fn current_masks_are_uploaded_once_and_navigation_releases_every_effect_texture() {
    for adapter in [
        crate::GpuAdapterPreference::Default,
        crate::GpuAdapterPreference::Software,
    ] {
        let device = DirectXDevices::with_instrumentation(adapter, Default::default()).unwrap();
        let mut r = offscreen(&device, W, H);
        r.group_tiles = false;
        r.gpu_timer.instrumentation = gpui::Instrumentation::new(gpui::InstrumentationConfig {
            gpu_timing: true,
            ..Default::default()
        });
        let many = scene(96, 0., 0, false);
        let expected = r
            .render_to_image(&many, WindowBackgroundAppearance::Transparent)
            .unwrap();
        assert!(take(&r).contains("mask_upload_create=96 mask_upload_alloc_bytes=24576"));
        let uploads: Vec<_> = r.blur.mask_cache.textures().cloned().collect();
        for _ in 0..3 {
            assert_eq!(
                r.render_to_image(&many, WindowBackgroundAppearance::Transparent)
                    .unwrap(),
                expected
            );
            assert!(!take(&r).contains("mask_upload_create=96"));
            assert_eq!(r.blur.mask_cache.textures().count(), 96);
            assert!(
                r.blur
                    .mask_cache
                    .textures()
                    .all(|texture| uploads.contains(texture))
            );
        }
        drop(uploads);
        let stage = r.blur.stage.as_ref().unwrap().texture.clone();
        let masks: Vec<_> = r.blur.mask_cache.textures().cloned().collect();
        r.rasterize_scene(&Scene::default(), size(DevicePixels(32), DevicePixels(24)))
            .unwrap();
        assert_eq!(r.blur.stage.as_ref().unwrap().texture, stage);
        assert!(
            r.blur
                .mask_cache
                .textures()
                .all(|texture| masks.contains(texture))
        );
        drop((stage, masks));
        assert!(!take(&r).contains("effect_target_drop=98"));
        let bytes = effect_bytes(&r);
        assert!(bytes > 24576);
        r.render_to_image(&Scene::default(), WindowBackgroundAppearance::Transparent)
            .unwrap();
        assert_eq!(effect_bytes(&r), 0);
        assert!(r.blur.group_mask.is_empty());
        let line = take(&r);
        assert!(
            line.contains(&format!(
                "effect_target_drop=98 effect_target_release_bytes={bytes}"
            )),
            "{line}"
        );
        assert_eq!(
            r.render_to_image(&many, WindowBackgroundAppearance::Transparent)
                .unwrap(),
            expected
        );
        assert!(take(&r).contains("mask_upload_create=96 mask_upload_alloc_bytes=24576"));
    }
}

#[test]
fn an_unchanged_frame_releases_masks_of_groups_that_no_longer_have_composites() {
    let device = DirectXDevices::new().unwrap();
    let mut r = offscreen(&device, W, H);
    let mut previous = scene(3, 0., 0, false);
    previous.surfaces.clear();
    previous.note_frame_damage(&Scene::default(), 1, (W, H));
    r.draw(&previous, WindowBackgroundAppearance::Transparent)
        .unwrap();
    assert_eq!(r.blur.mask_cache.textures().count(), 3);
    let mut next = Scene::default();
    next.note_frame_damage(&previous, 2, (W, H));
    assert_eq!(next.frame_damage().0, gpui::SceneDamage::Unchanged);
    r.draw(&next, WindowBackgroundAppearance::Transparent)
        .unwrap();
    assert_eq!(effect_bytes(&r), 0);
}

#[test]
fn removing_each_effect_kind_drops_its_scratch_without_changing_the_next_frame() {
    let device = DirectXDevices::new().unwrap();
    let mut r = offscreen(&device, W, H);
    r.group_tiles = true;
    assert!(!r.gpu_timer.instrumentation.config().gpu_timing);
    for (blur, blend, backdrop) in [(2., 1, true), (8., 1, true), (0., 0, false)] {
        let page = scene(3, blur, blend, backdrop);
        let mut cold = offscreen(&device, W, H);
        cold.group_tiles = true;
        let expected = cold
            .render_to_image(&page, WindowBackgroundAppearance::Transparent)
            .unwrap();
        assert_eq!(
            r.render_to_image(&page, WindowBackgroundAppearance::Transparent)
                .unwrap(),
            expected
        );
        assert_eq!(r.blur.gauss.is_some(), blur == 2.);
        assert_eq!(!r.blur.down.is_empty(), blur == 8. || backdrop);
        assert_eq!(r.blur.copy.is_some(), backdrop);
        if blend == 0 {
            assert_eq!(r.blur.level_backdrop.is_some(), r.group_tiles);
            assert!(r.blur.blend_copy.is_none());
            assert!(r.blur.pool.idle().is_empty());
            let backdrop = r.blur.level_backdrop.as_ref().unwrap().texture.clone();
            assert_eq!(
                r.render_to_image(&page, WindowBackgroundAppearance::Transparent)
                    .unwrap(),
                expected
            );
            assert_eq!(r.blur.level_backdrop.as_ref().unwrap().texture, backdrop);
        }
    }
    r.render_to_image(&Scene::default(), WindowBackgroundAppearance::Transparent)
        .unwrap();
    assert_eq!(effect_bytes(&r), 0);
    // Frame-plan diagnostics are independent of GPU timing. Only the new
    // resource events must remain absent while GPU timing is disabled.
    let line = take(&r);
    assert!(line.contains("mask_upload_create=0 mask_upload_alloc_bytes=0 effect_target_drop=0 effect_target_release_bytes=0"), "{line}");
}
