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

