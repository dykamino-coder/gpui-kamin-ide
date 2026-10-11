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

/// Armed wait fault for this view, consumed before any AcquireSync call. Main
/// deliberately keeps its generated `Result<()>` conversion for the injected
/// status (a positive WAIT_* is treated as acquired); PR #185 routes the same
/// fault into its exact-status branch instead of a separate fake path.
#[cfg(windows)]
pub(super) fn injected_wait(id: &str, keyed_mutex: bool) -> Option<i32> {
    keyed_mutex_presence(id, keyed_mutex);
    crate::native_acceptance::faults::wait_status(id)
}

/// Debug acceptance diagnostics: whether the producer's shared texture carries
/// a keyed mutex at all. Logged once per view and on every change, so a
/// capture shows which synchronization path (and wait-fault hook) is live.
#[cfg(windows)]
fn keyed_mutex_presence(id: &str, present: bool) {
    use std::collections::HashMap;
    use std::sync::{LazyLock, Mutex};
    static SEEN: LazyLock<Mutex<HashMap<String, bool>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));
    let Ok(mut seen) = SEEN.lock() else {
        return;
    };
    if seen.insert(id.to_string(), present) != Some(present) {
        acceptance_log(format!(
            "[acceptance] shared_texture view={id:?} keyed_mutex={present}"
        ));
    }
}
