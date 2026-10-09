//! Raw HRESULT на границе DXGI: generated AcquireSync().ok() теряет WAIT_*.

#[cfg(windows)]
pub(super) unsafe fn acquire(mutex: &windows::Win32::Graphics::Dxgi::IDXGIKeyedMutex) -> i32 {
    use windows::core::Interface;
    // SAFETY: живой COM interface, точная сигнатура vtable windows 0.62.
    unsafe { (mutex.vtable().AcquireSync)(mutex.as_raw(), 0, 16).0 }
}
