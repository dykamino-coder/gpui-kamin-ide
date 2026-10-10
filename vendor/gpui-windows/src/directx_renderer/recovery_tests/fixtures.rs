//! Headless renderer and populated device-bound effect fixtures.

use super::*;

pub(super) fn replacement(device: &DirectXDevices, width: u32, height: u32) -> RecoveryResources {
    let devices = DirectXRendererDevices::new(device, false).unwrap();
    RecoveryResources {
        resources: DirectXResources::new(&devices, width, height, HWND::default(), false).unwrap(),
        globals: DirectXGlobalElements::new(&devices.device).unwrap(),
        pipelines: DirectXRenderPipelines::new(&devices.device).unwrap(),
        devices,
        direct_composition: None,
    }
}

pub(super) fn renderer(device: &DirectXDevices) -> DirectXRenderer {
    let r = replacement(device, 16, 16);
    DirectXRenderer {
        hwnd: HWND::default(),
        atlas: Arc::new(DirectXAtlas::new(
            &r.devices.device,
            &r.devices.device_context,
        )),
        devices: Some(r.devices),
        resources: Some(r.resources),
        globals: r.globals,
        pipelines: r.pipelines,
        direct_composition: None,
        disable_direct_composition: false,
        font_info: &FontInfo {
            gamma_ratios: [1.; 4],
            grayscale_enhanced_contrast: 1.,
            subpixel_enhanced_contrast: 1.,
            is_bgr: false,
        },
        blur: BlurScratch::default(),
        group_target: None,
        group_target_tex: None,
        blend_premultiplied: None,
        blend_replace: None,
        width: 16,
        height: 16,
        skip_draws: false,
    }
}

pub(super) fn populate_effects(r: &mut DirectXRenderer) {
    let device = r.dev().device.clone();
    r.blur.copy = Some(create_blur_texture(&device, r.width, r.height, false).unwrap());
    r.blur.down.push(
        create_blur_texture(&device, (r.width / 2).max(1), (r.height / 2).max(1), true).unwrap(),
    );
    r.blur
        .groups
        .push(create_blur_texture(&device, r.width, r.height, true).unwrap());
    r.blur
        .mask_cache
        .insert(ImageId(7), r.blur.groups[0].srv.clone());
    r.blur
        .group_mask
        .push(Some((r.blur.groups[0].srv.clone(), [0.; 4], 0.)));
    r.blur.group_slots.push(0);
    r.blur.group_blend.push(1);
    r.blur.group_poly.push(([[0.; 4]; 4], 0));
    r.blur.group_clip.push([0.; 4]);
    r.ensure_blur_globals(&device).unwrap();
    r.group_target = Some(r.blur.groups[0].rtv.clone());
    r.group_target_tex = Some(r.blur.groups[0].texture.clone());
    r.premultiplied_blend().unwrap();
    r.replace_blend().unwrap();
}

pub(super) fn assert_effects_empty(r: &DirectXRenderer) {
    assert!(r.blur.copy.is_none());
    assert!(r.blur.down.is_empty() && r.blur.groups.is_empty());
    assert!(r.blur.globals[0].is_none());
    assert!(r.blur.mask_cache.is_empty() && r.blur.group_mask.is_empty());
    assert!(r.blur.group_slots.is_empty() && r.blur.group_blend.is_empty());
    assert!(r.blur.group_poly.is_empty() && r.blur.group_clip.is_empty());
    assert!(r.group_target.is_none() && r.group_target_tex.is_none());
    assert!(r.blend_premultiplied.is_none() && r.blend_replace.is_none());
}
