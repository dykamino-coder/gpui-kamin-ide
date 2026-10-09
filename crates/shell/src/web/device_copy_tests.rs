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
