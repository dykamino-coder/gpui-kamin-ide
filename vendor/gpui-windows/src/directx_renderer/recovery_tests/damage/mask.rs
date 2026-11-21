//! Sparse writes preserve clean pixels after MSAA and restore D3D state.
use super::*;
use crate::directx_renderer::{BatchParams, Region};
use gpui::{PathBuilder, px};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_CONSTANT_BUFFER, D3D11_BUFFER_DESC, D3D11_USAGE_DEFAULT,
};

fn software() -> DirectXDevices {
    DirectXDevices::with_instrumentation(
        crate::GpuAdapterPreference::Software,
        gpui::Instrumentation::default(),
    )
    .unwrap()
}

fn checkerboard(changed: bool) -> Scene {
    let mut scene = Scene::default();
    for y in 0..3 {
        for x in 0..4 {
            let edited = changed && (x + y) % 2 == 0;
            let mut q = quad(
                [x as f32 * 60. + 10., y as f32 * 60. + 10., 24., 24.],
                hsla(if edited { 0.75 } else { 0.15 }, 0.8, 0.5, 0.6),
            );
            q.order = y * 4 + x + 1;
            scene.quads.push(q);
        }
    }
    let mut builder = PathBuilder::fill();
    builder.move_to(point(px(0.), px(0.)));
    builder.line_to(point(px(W as f32), px(0.)));
    builder.line_to(point(px(W as f32), px(H as f32)));
    builder.line_to(point(px(0.), px(H as f32)));
    builder.close();
    let mut path = builder.build().unwrap().scale(1.);
    path.content_mask = mask();
    path.color = Background::from(hsla(0.4, 0.8, 0.4, 0.3));
    path.order = 20;
    scene.paths.push(path);
    // The batch after MSAA checks that restoring the frame RTV preserves stencil.
    let mut last = quad([0., 0., W as f32, H as f32], hsla(0.6, 0.7, 0.5, 0.2));
    last.order = 21;
    scene.quads.push(last);
    scene
}

#[test]
fn checkerboard_paths_and_later_blending_match_the_whole_frame() {
    let device = software();
    let mut retained = offscreen(&device, W, H);
    let mut previous = Scene::default();
    for (n, changed) in [false, true, false, true].into_iter().enumerate() {
        let mut scene = checkerboard(changed);
        scene.note_frame_damage(&previous, n as u64 + 1, (W, H));
        if n > 0 {
            let plan = retained.plan_frame(&scene, WindowBackgroundAppearance::Opaque);
            assert!(
                matches!(&plan, crate::directx_renderer::retained::Plan::Within(r) if r.len() >= 4)
            );
        }
        let before = retained.retained.scene_replays;
        retained
            .draw(&scene, WindowBackgroundAppearance::Opaque)
            .unwrap();
        assert_eq!(
            retained.retained.scene_replays - before,
            1,
            "one scene traversal"
        );
        let mut fresh = offscreen(&device, W, H);
        fresh
            .draw(&scene, WindowBackgroundAppearance::Opaque)
            .unwrap();
        let (got, want) = (target_bytes(&mut retained), target_bytes(&mut fresh));
        assert!(
            got == want,
            "frame {n}: {} pixels differ",
            differing(&got, &want)
        );
        assert!(!retained.retained.mask.active);
        if n > 0 {
            assert!(retained.retained.mask.texture().is_some());
        }
        previous = scene;
    }
    retained
        .resize(Size {
            width: DevicePixels(128),
            height: DevicePixels(96),
        })
        .unwrap();
    assert!(retained.retained.mask.texture().is_none());
    retained.reset_retained();
    assert!(retained.retained.mask.texture().is_none());
}

#[test]
fn sparse_clear_restores_frame_bindings_after_an_interrupted_group() {
    let device = software();
    let mut retained = offscreen(&device, W, H);
    let mut previous = checkerboard(false);
    previous.note_frame_damage(&Scene::default(), 1, (W, H));
    retained
        .draw(&previous, WindowBackgroundAppearance::Opaque)
        .unwrap();
    let mut scene = checkerboard(true);
    scene.note_frame_damage(&previous, 2, (W, H));
    let crate::directx_renderer::retained::Plan::Within(regions) =
        retained.plan_frame(&scene, WindowBackgroundAppearance::Opaque)
    else {
        panic!("expected sparse damage");
    };
    assert!(regions.len() >= 4);
    retained.pre_draw(&[1.; 4], false).unwrap();
    // A group may return Err before its normal frame-global restoration.
    // Poison both geometry and color bindings before clear draws mark the mask.
    let ctx = retained.dev().device_context.clone();
    let mut viewport = retained.res().viewport;
    viewport.Width = 1.;
    viewport.Height = 1.;
    unsafe {
        ctx.RSSetViewports(Some(std::slice::from_ref(&viewport)));
        ctx.VSSetConstantBuffers(0, Some(&[None, None]));
        ctx.PSSetConstantBuffers(0, Some(&[None]));
    }
    retained
        .draw_damage_regions(&scene, Some(&regions), [1.; 4])
        .unwrap();
    let mut fresh = offscreen(&device, W, H);
    fresh
        .draw(&scene, WindowBackgroundAppearance::Opaque)
        .unwrap();
    let (got, want) = (target_bytes(&mut retained), target_bytes(&mut fresh));
    assert!(
        got == want,
        "interrupted group: {} pixels differ",
        differing(&got, &want)
    );
}

#[test]
fn failed_mask_allocation_restores_rasterizer_and_depth_state() {
    let device = software();
    let mut renderer = offscreen(&device, W, H);
    let ctx = renderer.dev().device_context.clone();
    let rasterizer = unsafe { ctx.RSGetState() }.unwrap();
    let (mut depth, mut reference) = (None, 0);
    unsafe { ctx.OMGetDepthStencilState(Some(&mut depth), Some(&mut reference)) };
    // Zero dimensions deliberately violate the Texture2D contract and return a real HRESULT.
    renderer.width = 0;
    renderer.height = 0;
    let regions = [
        Region {
            x: 1,
            y: 1,
            width: 3,
            height: 3,
        },
        Region {
            x: 80,
            y: 80,
            width: 3,
            height: 3,
        },
    ];
    let result = renderer.draw_damage_regions(&Scene::default(), Some(&regions), [1.; 4]);
    assert!(result.is_err());
    assert!(!renderer.retained.mask.active);
    assert!(renderer.blur.frame_within.is_none());
    assert_eq!(unsafe { ctx.RSGetState() }.unwrap(), rasterizer);
    let (mut after, mut after_reference) = (None, 0);
    unsafe { ctx.OMGetDepthStencilState(Some(&mut after), Some(&mut after_reference)) };
    assert_eq!((after, after_reference), (depth, reference));
    (renderer.width, renderer.height) = (W, H);
    renderer
        .draw(&checkerboard(false), WindowBackgroundAppearance::Opaque)
        .unwrap();
}

#[test]
fn failed_scene_draw_restores_an_active_mask_before_the_next_frame() {
    let device = software();
    let mut renderer = offscreen(&device, W, H);
    let ctx = renderer.dev().device_context.clone();
    renderer.pre_draw(&[1.; 4], false).unwrap();
    let rasterizer = unsafe { ctx.RSGetState() }.unwrap();
    let (mut depth, mut reference) = (None, 0);
    unsafe { ctx.OMGetDepthStencilState(Some(&mut depth), Some(&mut reference)) };
    let desc = D3D11_BUFFER_DESC {
        ByteWidth: std::mem::size_of::<BatchParams>() as u32,
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
        ..Default::default()
    };
    let mut unmappable = None;
    unsafe {
        device
            .device
            .CreateBuffer(&desc, None, Some(&mut unmappable))
    }
    .unwrap();
    // Clear draws do not map b1; the first scene batch fails after the mask is active.
    let original = std::mem::replace(&mut renderer.globals.batch_params_buffer, unmappable);
    let regions = [
        Region {
            x: 1,
            y: 1,
            width: 3,
            height: 3,
        },
        Region {
            x: 80,
            y: 80,
            width: 3,
            height: 3,
        },
    ];
    let result = renderer.draw_damage_regions(&checkerboard(false), Some(&regions), [1.; 4]);
    renderer.globals.batch_params_buffer = original;
    assert!(result.is_err());
    assert!(
        renderer.retained.mask.texture().is_some(),
        "allocation succeeded"
    );
    assert!(!renderer.retained.mask.active);
    assert!(renderer.blur.frame_within.is_none());
    assert_eq!(unsafe { ctx.RSGetState() }.unwrap(), rasterizer);
    let (mut after, mut after_reference) = (None, 0);
    unsafe { ctx.OMGetDepthStencilState(Some(&mut after), Some(&mut after_reference)) };
    assert_eq!((after, after_reference), (depth, reference));
    let scene = checkerboard(false);
    renderer
        .draw(&scene, WindowBackgroundAppearance::Opaque)
        .unwrap();
    let mut fresh = offscreen(&device, W, H);
    fresh
        .draw(&scene, WindowBackgroundAppearance::Opaque)
        .unwrap();
    assert!(target_bytes(&mut renderer) == target_bytes(&mut fresh));
}
