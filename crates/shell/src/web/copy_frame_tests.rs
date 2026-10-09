//! Изолированный WARP unit gate: реальные D3D11/COM wait и cached-frame paths.

use crate::web::{copy_frame, gpu_texture, keyed_mutex};
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL};
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::{Common::*, IDXGIKeyedMutex};
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
fn real_warp_timeout_is_preserved_and_never_publishes_uninitialized_frame() {
    let (device, context) = device();
    let shared = texture(&device, true);
    let mutex: IDXGIKeyedMutex = shared.raw().cast().unwrap();
    unsafe {
        assert_eq!(keyed_mutex::acquire(&mutex), 0);
        mutex.ReleaseSync(1).unwrap();
        assert_eq!(keyed_mutex::acquire(&mutex), 258);
        mutex.AcquireSync(1, 0).unwrap();
        mutex.ReleaseSync(0).unwrap();
        mutex.AcquireSync(0, 0).unwrap();
        assert!(
            copy_frame::copy_into_own("first-busy", device.as_raw(), context.as_raw(), &shared)
                .is_none()
        );
        mutex.ReleaseSync(0).unwrap();
        let first =
            copy_frame::copy_into_own("first-busy", device.as_raw(), context.as_raw(), &shared)
                .unwrap();
        mutex.AcquireSync(0, 0).unwrap();
        let prior =
            copy_frame::copy_into_own("first-busy", device.as_raw(), context.as_raw(), &shared)
                .unwrap();
        assert!(prior.same_as(&first));
        mutex.ReleaseSync(0).unwrap();
    }
    copy_frame::forget_view("first-busy");
}
#[test]
fn missing_mutex_never_copies_or_publishes_a_frame() {
    let (device, context) = device();
    let shared = texture(&device, false);
    assert!(
        copy_frame::copy_into_own("no-key", device.as_raw(), context.as_raw(), &shared).is_none()
    );
}
