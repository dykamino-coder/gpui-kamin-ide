//! Debug-only facade keeps acceptance code outside private CEF internals.

pub(crate) fn acceptance_log(line: String) {
    super::diag_sink::emit_line(line);
}
pub(crate) fn acceptance_close(id: &str) {
    super::browsers::close(id);
}

pub(crate) fn reset_fixture() {
    acceptance_close(crate::native_acceptance::VIEW);
    let _ = std::fs::remove_file(
        crate::host_link::data_dirs()
            .1
            .join("webview-html")
            .join(format!("{}.html", crate::native_acceptance::VIEW)),
    );
}

/// Main's generated wrapper deliberately keeps its existing status conversion.
/// PR #185 uses `wait_status(id)` before its raw call instead: the same fault
/// must reach that PR's real exact-status branch, never a separate fake path.
#[cfg(windows)]
pub(super) unsafe fn acquire(
    id: &str,
    mutex: &windows::Win32::Graphics::Dxgi::IDXGIKeyedMutex,
) -> windows::core::Result<()> {
    if let Some(status) = crate::native_acceptance::faults::wait_status(id) {
        return windows::core::HRESULT(status).ok();
    }
    unsafe { mutex.AcquireSync(0, 16) }
}
