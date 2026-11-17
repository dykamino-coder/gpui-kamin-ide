//! Ошибки GPUI и платформы (`log::error!`) в `diag.log`.
//!
//! Логгера у шелла не было, и записи вендорного кода пропадали. Среди них
//! предел ожидания окон при выходе (INC-2026-0122): без записи зависший
//! выход не отличить от медленного CEF.

use log::{Level, LevelFilter, Log, Metadata, Record};

struct DiagLogger;

static LOGGER: DiagLogger = DiagLogger;

impl Log for DiagLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Error
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            super::emit_line(line(record));
        }
    }

    fn flush(&self) {}
}

fn line(record: &Record) -> String {
    format!(
        "[log] {} {}: {}",
        record.level(),
        record.target(),
        record.args()
    )
}

/// Подключить логгер; повторный вызов и чужой логгер оставляют прежний.
pub(crate) fn install() {
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(LevelFilter::Error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_errors_with_their_source_and_skips_warnings() {
        let error = Metadata::builder().level(Level::Error).build();
        let warning = Metadata::builder().level(Level::Warn).build();
        assert!(LOGGER.enabled(&error));
        assert!(!LOGGER.enabled(&warning));
        let line = line(
            &Record::builder()
                .level(Level::Error)
                .target("gpui_windows::platform::shutdown")
                .args(format_args!("2 window(s) still alive"))
                .build(),
        );
        assert_eq!(
            line,
            "[log] ERROR gpui_windows::platform::shutdown: 2 window(s) still alive"
        );
    }
}
