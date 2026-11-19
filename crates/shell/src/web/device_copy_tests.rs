//! Изолированный WARP unit gate: реальные D3D11/COM wait и cached-frame paths.

use crate::web::{copy_frame, gpu_texture};
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL};
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::Interface;
fn device() -> (ID3D11Device, ID3D11DeviceContext) {
    let (mut device, mut context) = (None, None);
    let mut level = D3D_FEATURE_LEVEL::default();
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_WARP,
            windows::Win32::Foundation::HMODULE::default(),
            D3D11_CREATE_DEVICE_FLAG(0),
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            Some(&mut level),
            Some(&mut context),
        )
        .unwrap();
    }
    (device.unwrap(), context.unwrap())
}
fn texture(device: &ID3D11Device, keyed: bool) -> gpu_texture::GpuTexture {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: 2,
        Height: 2,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_R8G8B8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
        CPUAccessFlags: 0,
        MiscFlags: if keyed {
            D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX.0 as u32
        } else {
            0
        },
    };
    let mut texture = None;
    unsafe {
        device
            .CreateTexture2D(&desc, None, Some(&mut texture))
            .unwrap();
        gpu_texture::GpuTexture::from_owned(texture.unwrap().into_raw()).unwrap()
    }
}
#[test]
fn rejects_old_source_or_foreign_context_and_replaces_equal_size_own_texture() {
    let (old, old_context) = device();
    let (new, new_context) = device();
    let old_source = texture(&old, true);
    let old_in_flight = old_source.clone();
    let first = copy_frame::copy_into_own(
        "generation-test",
        old.as_raw(),
        old_context.as_raw(),
        &old_source,
    )
    .unwrap();
    assert!(first.belongs_to(old.as_raw()));
    assert!(
        copy_frame::copy_into_own(
            "generation-test",
            old.as_raw(),
            new_context.as_raw(),
            &old_source
        )
        .is_none()
    );
    assert!(
        copy_frame::copy_into_own(
            "generation-test",
            new.as_raw(),
            new_context.as_raw(),
            &old_in_flight
        )
        .is_none()
    );
    let new_source = texture(&new, true);
    let second = copy_frame::copy_into_own(
        "generation-test",
        new.as_raw(),
        new_context.as_raw(),
        &new_source,
    )
    .unwrap();
    assert!(second.belongs_to(new.as_raw()));
    assert!(!second.belongs_to(old.as_raw()));
    assert!(!second.same_as(&first));
    copy_frame::forget_view("generation-test");
}

/// A copy with damage changes the own texture only inside the damage: CEF
/// changes unreported translucent texels between frames (INC-2026-0064).
#[test]
fn copy_with_damage_changes_only_the_damaged_texels() {
    let (device, context) = device();
    let source = texture(&device, false);
    let fill = |value: u8| {
        let texels = [value; 16];
        unsafe {
            context.UpdateSubresource(source.raw(), 0, None, texels.as_ptr().cast(), 8, 16);
        }
    };
    fill(10);
    let own = copy_frame::copy_into_own(
        "damage-copy-test",
        device.as_raw(),
        context.as_raw(),
        &source,
    )
    .unwrap();
    fill(200);
    let again = copy_frame::copy_damage_into_own(
        "damage-copy-test",
        device.as_raw(),
        context.as_raw(),
        &source,
        Some((&own, (1, 0, 5, 1))),
    )
    .unwrap();
    assert!(again.same_as(&own));
    let mut desc = D3D11_TEXTURE2D_DESC::default();
    unsafe { own.raw().GetDesc(&mut desc) };
    desc.Usage = D3D11_USAGE_STAGING;
    desc.BindFlags = 0;
    desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
    let mut staging = None;
    let red = unsafe {
        device
            .CreateTexture2D(&desc, None, Some(&mut staging))
            .unwrap();
        let staging = staging.unwrap();
        context.CopyResource(&staging, own.raw());
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        context
            .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
            .unwrap();
        let row = |y: usize| {
            std::slice::from_raw_parts(
                (mapped.pData as *const u8).add(y * mapped.RowPitch as usize),
                8,
            )
            .to_vec()
        };
        let texels = [row(0), row(1)].map(|r| [r[0], r[4]]);
        context.Unmap(&staging, 0);
        texels
    };
    // Damage (1, 0, 5, 1) clamps to texel (1, 0) of the 2 x 2 buffer.
    assert_eq!(red, [[10, 200], [10, 10]]);
    copy_frame::forget_view("damage-copy-test");
}
