//! Allocation-free presence check before opt-in configuration is read.

pub(super) const AT: &str = "ENGINE_BENCH_RETAINERS_AT";

/// Only read after retention tracking has been explicitly enabled.
pub(super) fn target() -> Result<Option<(u64, usize)>, String> {
    let Some(text) = std::env::var_os(AT) else {
        return Ok(None);
    };
    let text = text.to_string_lossy();
    let parsed = text.split_once('@').and_then(|(phase, frame)| {
        let phase = kamin_html::work::Phase::NAMES
            .iter()
            .position(|name| *name == phase)?;
        let frame = frame.parse::<u64>().ok().filter(|frame| *frame > 0)?;
        Some((frame, phase))
    });
    parsed
        .map(Some)
        .ok_or_else(|| format!("{AT}={text}: expected <phase>@<positive frame>"))
}

#[cfg(windows)]
pub(super) fn present() -> bool {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetEnvironmentVariableW(name: *const u16, buffer: *mut u16, size: u32) -> u32;
        fn SetLastError(error: u32);
        fn GetLastError() -> u32;
    }
    let mut name = [0u16; super::OUT.len() + 1];
    for (unit, byte) in name.iter_mut().zip(super::OUT.bytes()) {
        *unit = u16::from(byte);
    }
    let mut value = 0u16;
    // SAFETY: the name is nul-terminated and the output buffer holds one UTF-16 unit.
    // An empty but present value remains enabled, matching std::env::var_os.
    unsafe {
        SetLastError(0);
        GetEnvironmentVariableW(name.as_ptr(), &mut value, 1) != 0 || GetLastError() == 0
    }
}

#[cfg(unix)]
pub(super) fn present() -> bool {
    unsafe extern "C" {
        fn getenv(name: *const std::ffi::c_char) -> *const std::ffi::c_char;
    }
    // SAFETY: the constant name is nul-terminated; the borrowed value is never read or retained.
    unsafe { !getenv(c"ENGINE_BENCH_RETAINERS_OUT".as_ptr()).is_null() }
}

#[cfg(not(any(windows, unix)))]
pub(super) fn present() -> bool {
    std::env::var_os(super::OUT).is_some()
}
