//! KaminIDE patch: an offscreen target is the swap chain back buffer's twin.
//! Headless: no HWND is created and nothing is presented.

use super::*;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, D3D11_TEXTURE2D_DESC, ID3D11Texture2D,
};

#[test]
fn cold_renderer_reports_device_work_before_the_first_frame() {
    let instrumentation = gpui::Instrumentation::new(gpui::InstrumentationConfig {
        startup: true,
        ..Default::default()
    });
    let device = DirectXDevices::with_instrumentation(
        crate::GpuAdapterPreference::Software,
        instrumentation.clone(),
    ).unwrap();
    let _renderer = DirectXRenderer::new_offscreen(HWND::default(), &device).unwrap();
    let timings = instrumentation.startup.take();
    for name in ["renderer.create", "renderer.surface", "renderer.globals", "renderer.pipelines"] {
        assert_eq!(timings[name].calls, 1, "{name}");
    }
    assert!(!timings.contains_key("renderer.vertex_shader_create"));
    assert!(!timings.contains_key("renderer.pixel_shader_create"));
    assert!(instrumentation.startup.take().is_empty());
}

fn desc(texture: &ID3D11Texture2D) -> D3D11_TEXTURE2D_DESC {
    let mut desc = D3D11_TEXTURE2D_DESC::default();
    unsafe { texture.GetDesc(&mut desc) };
    desc
}

#[test]
fn offscreen_target_matches_the_swap_chain_back_buffer() {
    let device = DirectXDevices::new().unwrap();
    let presented = replacement(&device, 24, 16);
    let back = desc(presented.resources.render_target.as_ref().unwrap());
    let r = offscreen(&device, 24, 16);
    let target = desc(r.res().render_target.as_ref().unwrap());
    assert!(r.res().swap_chain.is_none() && r.res().frame_gate.is_none());
    assert_eq!(
        (target.Width, target.Height, target.Format, target.MipLevels, target.ArraySize),
        (back.Width, back.Height, back.Format, back.MipLevels, back.ArraySize)
    );
    assert_eq!(
        (target.SampleDesc.Count, target.SampleDesc.Quality),
        (back.SampleDesc.Count, back.SampleDesc.Quality)
    );
    assert_eq!(target.BindFlags & D3D11_BIND_RENDER_TARGET.0 as u32, D3D11_BIND_RENDER_TARGET.0 as u32);
}

#[test]
fn offscreen_frames_draw_resize_and_read_back_without_presenting() {
    let device = DirectXDevices::new().unwrap();
    let mut presented = renderer(&device);
    let mut r = offscreen(&device, 16, 16);
    let scene = Scene::default();
    let a = presented.render_to_image(&scene, WindowBackgroundAppearance::Opaque).unwrap();
    let b = r.render_to_image(&scene, WindowBackgroundAppearance::Opaque).unwrap();
    assert_eq!(a.as_raw(), b.as_raw());
    r.draw(&scene, WindowBackgroundAppearance::Opaque).unwrap();
    r.resize(Size { width: DevicePixels(40), height: DevicePixels(8) }).unwrap();
    let target = desc(r.res().render_target.as_ref().unwrap());
    assert_eq!((target.Width, target.Height), (40, 8));
    let image = r.render_to_image(&scene, WindowBackgroundAppearance::Opaque).unwrap();
    assert_eq!(image.dimensions(), (40, 8));
    assert!(image.pixels().all(|p| p.0 == [255; 4]));
}

#[test]
fn offscreen_recovery_rebuilds_an_offscreen_target() {
    let device = DirectXDevices::new().unwrap();
    let mut r = offscreen(&device, 16, 16);
    r.handle_device_lost_impl(&device).unwrap();
    assert!(r.res().swap_chain.is_none() && r.direct_composition.is_none());
    assert_eq!(desc(r.res().render_target.as_ref().unwrap()).Width, 16);
}

#[test]
fn the_gpu_wait_sleeps_on_a_fence_and_leaves_the_frame_as_drawn() {
    use super::super::gpu_wait::GpuWait;
    use windows::core::Interface;
    let device = DirectXDevices::new().unwrap();
    let mut r = offscreen(&device, 16, 16);
    let scene = Scene::default();
    let a = r.render_to_image(&scene, WindowBackgroundAppearance::Opaque).unwrap();
    r.draw(&scene, WindowBackgroundAppearance::Opaque).unwrap();
    r.wait_gpu_idle().unwrap();
    r.wait_gpu_idle().unwrap();
    let fences = r
        .dev()
        .device
        .cast::<windows::Win32::Graphics::Direct3D11::ID3D11Device5>()
        .is_ok();
    match &r.retained.gpu_wait {
        GpuWait::Fence(_, value, _) => assert_eq!(*value, 2, "one signal per wait"),
        GpuWait::Query => assert!(!fences, "a device with fences waits on a fence"),
        GpuWait::Unknown => panic!("the first wait chooses how to wait"),
    }
    let b = r.render_to_image(&scene, WindowBackgroundAppearance::Opaque).unwrap();
    assert_eq!(a.as_raw(), b.as_raw());
    r.handle_device_lost_impl(&device).unwrap();
    assert!(matches!(r.retained.gpu_wait, GpuWait::Unknown), "a new device chooses again");
}
