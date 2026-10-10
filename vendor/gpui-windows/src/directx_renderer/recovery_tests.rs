//! Headless COM ownership regressions; never create a HWND or present a frame.

use super::recovery::RecoveryResources;
use super::{
    BUFFER_COUNT, BlurScratch, DirectXAtlas, DirectXDevices, DirectXGlobalElements,
    DirectXRenderPipelines, DirectXRenderer, DirectXRendererDevices, DirectXResources, FontInfo,
    RENDER_TARGET_FORMAT, create_blur_texture, create_path_intermediate_msaa_texture_and_view,
    create_path_intermediate_texture, create_render_target_and_its_view,
    try_to_recover_from_device_lost,
};
use gpui::{DevicePixels, ImageId, Scene, Size, WindowBackgroundAppearance};
use std::sync::Arc;
use windows::Win32::{
    Foundation::HWND, Graphics::Dxgi::DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT,
};

mod fixtures;
use fixtures::*;

#[test]
fn effects_follow_device_generation_after_repeated_recovery() {
    let a = DirectXDevices::new().unwrap();
    let b = DirectXDevices::new().unwrap();
    assert_ne!(a.device, b.device);
    let mut r = renderer(&a);
    for (width, height, effects, next) in
        [(16, 16, true, &b), (32, 24, true, &a), (32, 24, false, &b)]
    {
        if effects {
            populate_effects(&mut r);
        }
        r.width = width;
        r.height = height;
        r.recover_with(|disabled| {
            assert!(!disabled);
            Ok(replacement(next, width, height))
        })
        .unwrap();
        assert_effects_empty(&r);
        assert_eq!(r.dev().device, next.device);
        populate_effects(&mut r);
        assert_eq!(
            (r.blur.groups[0].width, r.blur.groups[0].height),
            (width, height)
        );
        let copy_device = unsafe { r.blur.copy.as_ref().unwrap().texture.GetDevice() }.unwrap();
        let group_device = unsafe { r.blur.groups[0].texture.GetDevice() }.unwrap();
        let global_device = unsafe { r.blur.globals[0].as_ref().unwrap().GetDevice() }.unwrap();
        let blend_device = unsafe { r.blend_replace.as_ref().unwrap().GetDevice() }.unwrap();
        assert_eq!(copy_device, next.device);
        assert_eq!(group_device, next.device);
        assert_eq!(global_device, next.device);
        assert_eq!(blend_device, next.device);
        assert_eq!(
            unsafe { r.blur.down[0].texture.GetDevice() }.unwrap(),
            next.device
        );
        assert_eq!(
            unsafe { r.blend_premultiplied.as_ref().unwrap().GetDevice() }.unwrap(),
            next.device
        );
        r.clear_effect_resources();
    }
}

#[test]
fn failed_recovery_releases_slots_and_preserves_composition_policy() {
    let device = DirectXDevices::new().unwrap();
    let mut r = renderer(&device);
    populate_effects(&mut r);
    // Fail after each replacement stage; locals unwind with normal COM Drop.
    for stage in 0..6 {
        let result = r.recover_with(|disabled| {
            assert!(!disabled);
            if stage == 0 {
                anyhow::bail!("injected devices failure");
            }
            let devices = DirectXRendererDevices::new(&device, disabled)?;
            if stage == 1 {
                anyhow::bail!("injected resources failure");
            }
            let _resources = DirectXResources::new(&devices, 16, 16, HWND::default(), false)?;
            if stage == 2 {
                anyhow::bail!("injected globals failure");
            }
            let _globals = DirectXGlobalElements::new(&devices.device)?;
            if stage == 3 {
                anyhow::bail!("injected pipelines failure");
            }
            let _pipelines = DirectXRenderPipelines::new(&devices.device)?;
            anyhow::bail!("injected composition/set_swap_chain failure at {stage}")
        });
        assert!(result.is_err());
        assert!(r.devices.is_none() && r.resources.is_none());
        assert_effects_empty(&r);
        assert!(r.skip_draws);
        assert!(r.d3d_device_raw().is_none() && r.d3d_context_raw().is_none());
        r.mark_drawable();
        assert!(
            r.draw(&Scene::default(), WindowBackgroundAppearance::Opaque)
                .is_err()
        );
    }
    r.recover_with(|disabled| {
        assert!(!disabled);
        Ok(replacement(&device, 16, 16))
    })
    .unwrap();
    assert!(r.ensure_drawable_resources().is_ok());
}

#[test]
fn failed_resize_is_unavailable_and_same_or_different_size_retry_repairs_it() {
    let device = DirectXDevices::new().unwrap();
    let mut r = renderer(&device);
    for (size, stage) in [(16, 0), (24, 1), (24, 2), (32, 3), (32, 4)] {
        r.width = size;
        r.height = size;
        r.resize_with(|resources, devices| {
            if stage == 0 {
                anyhow::bail!("injected ResizeBuffers failure");
            }
            unsafe {
                resources.swap_chain.ResizeBuffers(
                    BUFFER_COUNT as u32,
                    size,
                    size,
                    RENDER_TARGET_FORMAT,
                    DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT,
                )?;
            }
            if stage == 1 {
                anyhow::bail!("injected target failure");
            }
            let _target =
                create_render_target_and_its_view(&resources.swap_chain, &devices.device)?;
            if stage == 2 {
                anyhow::bail!("injected intermediate failure");
            }
            let _path = create_path_intermediate_texture(&devices.device, size, size)?;
            if stage == 3 {
                anyhow::bail!("injected MSAA failure");
            }
            let _msaa =
                create_path_intermediate_msaa_texture_and_view(&devices.device, size, size)?;
            anyhow::bail!("injected failure after partial construction")
        })
        .unwrap_err();
        assert!(r.res().render_target.is_none() && r.res().render_target_view.is_none());
        assert!(
            r.draw(&Scene::default(), WindowBackgroundAppearance::Opaque)
                .is_err()
        );
        r.resize(Size {
            width: DevicePixels(size as i32),
            height: DevicePixels(size as i32),
        })
        .unwrap();
        assert!(r.ensure_drawable_resources().is_ok());
        assert_eq!(
            (r.res().viewport.Width, r.res().viewport.Height),
            (size as f32, size as f32)
        );
    }
    r.resize_with(|_, _| anyhow::bail!("injected final failure"))
        .unwrap_err();
    r.resize(Size {
        width: DevicePixels(48),
        height: DevicePixels(24),
    })
    .unwrap();
    assert!(r.ensure_drawable_resources().is_ok());
    r.resize_with(|_, _| anyhow::bail!("injected final failure"))
        .unwrap_err();
    drop(r); // Teardown of an unavailable renderer must not release dead slots.
}

#[test]
fn exhausted_recovery_can_retry_from_empty_owners_and_teardown() {
    let device = DirectXDevices::new().unwrap();
    for disabled in [false, true] {
        let mut r = renderer(&device);
        r.disable_direct_composition = disabled;
        let mut attempts = 0;
        let error = try_to_recover_from_device_lost(|| {
            r.recover_with(|policy| {
                assert_eq!(policy, disabled);
                attempts += 1;
                anyhow::bail!("injected allocation failure")
            })
        })
        .unwrap_err();
        assert!(error.to_string().contains("multiple attempts"));
        assert_eq!(attempts, 5);
        assert!(r.devices.is_none() && r.resources.is_none());
        assert!(r.skip_draws);
        assert_effects_empty(&r);
        r.recover_with(|policy| {
            assert_eq!(policy, disabled);
            Ok(replacement(&device, 16, 16))
        })
        .unwrap();
        r.mark_drawable();
        assert!(r.ensure_drawable_resources().is_ok());
        r.recover_with(|_| anyhow::bail!("injected final failure"))
            .unwrap_err();
        drop(r);
    }
}
