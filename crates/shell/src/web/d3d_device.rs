//! Владение D3D interop: GPUI getters возвращают AddRef, borrowed calls его не
//! забирают. Сильный snapshot защищает CEF open от конкурентной смены device.

#[cfg(windows)]
use windows::Win32::Graphics::Direct3D11::ID3D11Device;
#[cfg(windows)]
use windows::core::Interface;

#[cfg(windows)]
pub(super) unsafe fn take_owned<T: Interface>(raw: Option<*mut std::ffi::c_void>) -> Option<T> {
    let raw = raw.filter(|raw| !raw.is_null())?;
    // SAFETY: caller передаёт ровно одну owned ссылку от соответствующего getter.
    Some(unsafe { T::from_raw(raw) })
}

#[cfg(windows)]
#[derive(Clone)]
pub(crate) struct Device(ID3D11Device);

#[cfg(windows)]
impl PartialEq for Device {
    fn eq(&self, other: &Self) -> bool {
        self.raw() == other.raw()
    }
}

#[cfg(windows)]
// SAFETY: D3D11 device создан без SINGLETHREADED; context сюда не входит.
unsafe impl Send for Device {}
#[cfg(windows)]
unsafe impl Sync for Device {}

#[cfg(windows)]
impl Device {
    pub(super) unsafe fn from_owned(raw: Option<*mut std::ffi::c_void>) -> Option<Self> {
        unsafe { take_owned(raw).map(Self) }
    }
    pub(super) fn raw(&self) -> *mut std::ffi::c_void {
        self.0.as_raw()
    }
}

#[cfg(all(test, windows))]
#[path = "d3d_device_tests.rs"]
mod tests;
