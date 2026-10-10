//! Application-scoped D3D11On12 loss; GPUI's existing monitor recreates devices.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use windows::Win32::Graphics::{
    Direct3D11::ID3D11Device, Direct3D11on12::ID3D11On12Device1, Direct3D12::ID3D12Device5,
    Dxgi::IDXGIDevice,
};
use windows::core::Interface;

static PENDING: AtomicBool = AtomicBool::new(false);
static LAST: AtomicUsize = AtomicUsize::new(0);

pub(crate) fn request() {
    PENDING.store(true, Ordering::Release);
}

pub(crate) fn observe(window: &gpui::Window) {
    if !super::active() {
        return;
    }
    let Some(raw) = window.d3d_device_raw().filter(|raw| !raw.is_null()) else {
        return;
    };
    // GPUI returns one owned AddRef. This seam balances its own getter even
    // when the debug interface is unavailable or no injection is pending.
    let device = unsafe { ID3D11Device::from_raw(raw) };
    let previous = LAST.swap(raw as usize, Ordering::Relaxed);
    if previous != raw as usize {
        let adapter = unsafe {
            device
                .cast::<IDXGIDevice>()
                .and_then(|d| d.GetAdapter())
                .and_then(|a| a.GetDesc())
        };
        crate::web::acceptance_log(format!(
            "[acceptance] device_changed={} adapter={:?} vendor={:?}",
            previous != 0,
            adapter
                .as_ref()
                .ok()
                .map(|a| String::from_utf16_lossy(&a.Description)
                    .trim_end_matches('\0')
                    .to_string()),
            adapter.as_ref().ok().map(|a| a.VendorId)
        ));
    }
    if !PENDING.swap(false, Ordering::AcqRel) {
        return;
    }
    let result = unsafe {
        device
            .cast::<ID3D11On12Device1>()
            .and_then(|on12| on12.GetD3D12Device::<ID3D12Device5>())
    };
    match result {
        Ok(on12) => {
            unsafe {
                on12.RemoveDevice();
            }
            crate::web::acceptance_log(
                "[acceptance] device_loss=removed awaiting_gpui_recreation=true".into(),
            );
        }
        Err(_) => crate::web::acceptance_log(
            "[acceptance] device_loss=unsupported requires_D3D11On12=true".into(),
        ),
    }
}
