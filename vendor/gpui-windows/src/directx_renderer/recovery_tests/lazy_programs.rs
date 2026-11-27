//! Cold draws create only the programs they use, including on WARP.
use super::*;
use gpui::{Background, ContentMask, Primitive, Quad, ScaledPixels, bounds, point, size};

fn scene() -> Scene {
    let mut scene = Scene::default();
    for (x, width, color) in [(0., 16., 0xffffffff), (4., 8., 0xff0000ff)] {
        let bounds = bounds(
            point(ScaledPixels(x), ScaledPixels(0.)),
            size(ScaledPixels(width), ScaledPixels(16.)),
        );
        scene.insert_primitive(Primitive::Quad(Quad {
            bounds,
            content_mask: ContentMask { bounds },
            background: Background::from(gpui::rgba(color)),
            ..Default::default()
        }));
    }
    scene.finish();
    scene
}

// The eager reference reproduces the previous program ownership.
fn eagerly_create(r: &DirectXRenderer) -> Vec<String> {
    let mut identities = Vec::new();
    macro_rules! pipeline {
        ($($name:ident),+ $(,)?) => {$({
            let p = &r.pipelines.$name;
            p.vertex.get(&r.dev().device_context).unwrap();
            p.fragment.get(&r.dev().device_context).unwrap();
            identities.extend(p.shader_executables.iter().cloned());
        })+};
    }
    pipeline!(
        shadow_pipeline,
        quad_pipeline,
        path_rasterization_pipeline,
        path_sprite_pipeline,
        underline_pipeline,
        mono_sprites,
        subpixel_sprites,
        poly_sprites,
        blur_pipeline,
        clear_pipeline,
        group_tiles_pipeline
    );
    identities
}

fn created(profile: &gpui::StartupProfile, expected: u64) {
    let timings = profile.take();
    for name in [
        "renderer.vertex_shader_create",
        "renderer.pixel_shader_create",
    ] {
        assert_eq!(timings.get(name).map_or(0, |v| v.calls), expected, "{name}");
    }
}

#[test]
fn cold_quad_creates_only_used_programs_and_recovery_repeats_that_work() {
    use crate::GpuAdapterPreference as P;
    for preference in [P::Software, P::Default, P::HighPerformance] {
        let instrumentation = gpui::Instrumentation::new(gpui::InstrumentationConfig {
            startup: true,
            ..Default::default()
        });
        let device =
            match DirectXDevices::with_instrumentation(preference.clone(), instrumentation.clone())
            {
                Ok(device) => device,
                Err(error) if preference != P::Software => {
                    eprintln!("adapter unavailable: {error}");
                    continue;
                }
                Err(error) => panic!("WARP required: {error}"),
            };
        let mut lazy = offscreen(&device, 16, 16);
        created(&instrumentation.startup, 0);
        let scene = scene();
        let first = lazy
            .render_to_image(&scene, WindowBackgroundAppearance::Opaque)
            .unwrap();
        // One clear and one quad; the other nine pipelines have no programs yet.
        created(&instrumentation.startup, 2);
        for (x, _, pixel) in first.enumerate_pixels() {
            assert_eq!(
                pixel.0,
                if (4..12).contains(&x) {
                    [255, 0, 0, 255]
                } else {
                    [255; 4]
                }
            );
        }
        let again = lazy
            .render_to_image(&scene, WindowBackgroundAppearance::Opaque)
            .unwrap();
        assert_eq!(first.as_raw(), again.as_raw());
        created(&instrumentation.startup, 0);

        let mut eager = offscreen(&device, 16, 16);
        let identities = eagerly_create(&eager);
        created(&instrumentation.startup, 11);
        let reference = eager
            .render_to_image(&scene, WindowBackgroundAppearance::Opaque)
            .unwrap();
        assert_eq!(first.as_raw(), reference.as_raw());
        created(&instrumentation.startup, 0);

        let replacement =
            DirectXDevices::with_instrumentation(preference, instrumentation.clone()).unwrap();
        assert_ne!(device.device, replacement.device);
        lazy.handle_device_lost_impl(&replacement).unwrap();
        created(&instrumentation.startup, 0);
        assert!(lazy.skip_draws);
        lazy.ensure_drawable_resources().unwrap();
        // The platform resumes drawing after every renderer has recovered.
        lazy.mark_drawable();
        let recovered = lazy
            .render_to_image(&scene, WindowBackgroundAppearance::Opaque)
            .unwrap();
        assert_eq!(first.as_raw(), recovered.as_raw());
        created(&instrumentation.startup, 2);
        assert_eq!(eagerly_create(&lazy), identities);
        created(&instrumentation.startup, 9);
    }
}
