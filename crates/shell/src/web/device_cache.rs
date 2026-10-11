//! Сильный CEF snapshot и atomic generation commit; порядок замков device→frame.

use super::d3d_device::Device;
use super::device_generation::Current;
use std::sync::{LazyLock, Mutex};

static DEVICE: LazyLock<Mutex<Current<Device>>> = LazyLock::new(|| Mutex::new(Current::default()));

pub(crate) fn device() -> Option<Device> {
    DEVICE.lock().ok()?.snapshot()
}

/// Callback старого device не может закоммитить кадр после очистки новой генерации.
pub(crate) fn with_current_device<T>(device: &Device, commit: impl FnOnce() -> T) -> Option<T> {
    let current = DEVICE.lock().ok()?;
    if !current.matches(device) {
        return None;
    }
    Some(commit())
}

pub(crate) fn remember_device(window: &gpui::Window) {
    // SAFETY: GPUI getter отдаёт owned clone().into_raw().
    if let Some(device) = unsafe { Device::from_owned(window.d3d_device_raw()) } {
        sync_device(device);
    }
}

pub(super) fn sync_device(device: Device) {
    if let Ok(mut current) = DEVICE.lock() {
        current.observe(device, super::shared_texture::forget_device_objects);
    }
}
