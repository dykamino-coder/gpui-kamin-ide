//! KaminIDE patch: allocation follows actual path work; mutable scenes, resize,
//! recovery and embedded captures retain independent, correctly sized owners.

use super::*;
use gpui::{Background, Bounds, ContentMask, PaintGroup, Path, Pixels, hsla, point, px, size};

fn path_scene() -> Scene {
    let mut path = Path::<Pixels>::new(point(px(4.25), px(3.5)));
    path.line_to(point(px(45.5), px(6.25)));
    path.curve_to(point(px(28.), px(35.75)), point(px(58.), px(26.)));
    path.line_to(point(px(4.25), px(3.5)));
    path.color = Background::from(hsla(0.6, 0.7, 0.4, 0.8));
    path.content_mask = ContentMask {
        bounds: Bounds::new(point(px(0.), px(0.)), size(px(64.), px(48.))),
    };
    let mut scene = Scene::default();
    scene.paths.push(path.scale(1.));
    scene
}

fn path_bytes(renderer: &DirectXRenderer) -> u64 {
    let mut bytes = 0;
    renderer.gpu_memory(&mut |owner, allocated, _| {
        if owner == "gpu/target/path-msaa" {
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

fn instrumented(device: &DirectXDevices) -> DirectXRenderer {
    let mut renderer = offscreen(device, 64, 48);
    renderer.gpu_timer.instrumentation = gpui::Instrumentation::new(gpui::InstrumentationConfig {
        gpu_timing: true,
        ..Default::default()
    });
    renderer
}

#[test]
fn mutable_paths_allocate_once_release_and_reappear_with_exact_pixels() {
    for adapter in [
        crate::GpuAdapterPreference::Default,
        crate::GpuAdapterPreference::Software,
    ] {
        let device = DirectXDevices::with_instrumentation(adapter, Default::default()).unwrap();
        let mut r = instrumented(&device);
        let empty = Scene::default();
        assert_eq!(path_bytes(&r), 0);
        r.render_to_image(&empty, WindowBackgroundAppearance::Transparent)
            .unwrap();
        assert_eq!(path_bytes(&r), 0);
        assert!(!take(&r).contains("path_msaa_create=1"));

        let scene = path_scene();
        // The eager reference uses exactly the former resource creation path.
        let mut eager = instrumented(&device);
        eager.ensure_path_target().unwrap();
        let expected = eager
            .render_to_image(&scene, WindowBackgroundAppearance::Transparent)
            .unwrap();
        let first = r
            .render_to_image(&scene, WindowBackgroundAppearance::Transparent)
            .unwrap();
        assert_eq!(first, expected);
        assert_eq!(path_bytes(&r), 16 * 64 * 48);
        let line = take(&r);
        assert!(
            line.contains("path_msaa_create=1 path_msaa_drop=0 path_msaa_alloc_bytes=49152"),
            "{line}"
        );
        let texture = r.res().path_intermediate_msaa_texture.clone();
        assert_eq!(
            r.render_to_image(&scene, WindowBackgroundAppearance::Transparent)
                .unwrap(),
            expected
        );
        assert_eq!(r.res().path_intermediate_msaa_texture, texture);
        assert!(!take(&r).contains("path_msaa_create=1"));

        r.render_to_image(&empty, WindowBackgroundAppearance::Transparent)
            .unwrap();
        assert_eq!(path_bytes(&r), 0);
        assert!(take(&r).contains("path_msaa_drop=1"));
        assert_eq!(
            r.render_to_image(&scene, WindowBackgroundAppearance::Transparent)
                .unwrap(),
            expected
        );
        assert!(take(&r).contains("path_msaa_create=1"));
    }
}

#[test]
fn empty_path_batches_never_allocate_and_nested_paths_keep_the_owner() {
    let device = DirectXDevices::new().unwrap();
    let mut r = instrumented(&device);
    let mut clipped = path_scene();
    clipped.paths[0].content_mask.bounds.size =
        size(gpui::ScaledPixels(0.), gpui::ScaledPixels(0.));
    r.render_to_image(&clipped, WindowBackgroundAppearance::Transparent)
        .unwrap();
    assert_eq!(path_bytes(&r), 0);

    r.ensure_path_target().unwrap();
    let texture = r.res().path_intermediate_msaa_texture.clone();
    let mut outer = Scene::default();
    outer.groups.push(PaintGroup {
        scene: std::rc::Rc::new(path_scene()),
        ..Default::default()
    });
    r.release_unused_path_target(&outer);
    assert_eq!(r.res().path_intermediate_msaa_texture, texture);
    outer.groups[0].scene = std::rc::Rc::new(Scene::default());
    r.release_unused_path_target(&outer);
    assert_eq!(path_bytes(&r), 0);
}

#[test]
fn embedded_capture_resize_and_failed_recovery_release_only_their_targets() {
    let device = DirectXDevices::new().unwrap();
    let mut r = instrumented(&device);
    r.render_to_image(&path_scene(), WindowBackgroundAppearance::Transparent)
        .unwrap();
    let texture = r.res().path_intermediate_msaa_texture.clone();
    take(&r);
    r.rasterize_scene(&Scene::default(), size(DevicePixels(32), DevicePixels(24)))
        .unwrap();
    assert_eq!(r.res().path_intermediate_msaa_texture, texture);
    assert!(!take(&r).contains("path_msaa_create=1"));
    r.rasterize_scene(&path_scene(), size(DevicePixels(32), DevicePixels(24)))
        .unwrap();
    assert_eq!(r.res().path_intermediate_msaa_texture, texture);
    let line = take(&r);
    assert!(
        line.contains("path_msaa_create=1 path_msaa_drop=1 path_msaa_alloc_bytes=12288"),
        "{line}"
    );
    r.resize(size(DevicePixels(32), DevicePixels(24))).unwrap();
    assert_eq!(path_bytes(&r), 0);
    assert!(take(&r).contains("path_msaa_drop=1"));
    r.render_to_image(&path_scene(), WindowBackgroundAppearance::Transparent)
        .unwrap();
    assert_eq!(path_bytes(&r), 16 * 32 * 24);
    take(&r);
    assert!(
        r.recover_with(|_| anyhow::bail!("injected resource failure"))
            .is_err()
    );
    assert!(r.resources.is_none());
    assert!(take(&r).contains("path_msaa_drop=1"));
}

#[test]
fn disabled_instrumentation_does_not_record_resource_events() {
    let device = DirectXDevices::new().unwrap();
    let mut r = offscreen(&device, 64, 48);
    r.ensure_path_target().unwrap();
    assert_eq!(path_bytes(&r), 16 * 64 * 48);
    r.release_path_target();
    assert_eq!(path_bytes(&r), 0);
    assert!(r.gpu_timer.instrumentation.batches.take_line().is_none());
}

#[test]
fn renderer_drop_records_the_last_owner_release() {
    let device = DirectXDevices::new().unwrap();
    let mut r = instrumented(&device);
    r.ensure_path_target().unwrap();
    take(&r);
    let instrumentation = r.gpu_timer.instrumentation.clone();
    drop(r);
    assert!(
        instrumentation
            .batches
            .take_line()
            .unwrap()
            .contains("path_msaa_drop=1")
    );
}
