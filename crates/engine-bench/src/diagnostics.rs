//! Ошибки backend остаются видимыми при незавершённом измерении.

struct Stderr;

static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

impl log::Log for Stderr {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Warn
            || (metadata.target() == "gpui::frame" && log::max_level() >= log::LevelFilter::Debug)
    }

    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            eprintln!(
                "{:.6} {} {} {}:{}: {}",
                START
                    .get()
                    .map_or(0.0, |start| start.elapsed().as_secs_f64()),
                record.level(),
                record.target(),
                record.file().unwrap_or("?"),
                record.line().unwrap_or(0),
                record.args()
            );
        }
    }

    fn flush(&self) {}
}

pub(super) fn init() {
    START.get_or_init(std::time::Instant::now);
    // Логгер принадлежит executable; stdout остаётся потоком отчётов.
    static LOGGER: Stderr = Stderr;
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(log::LevelFilter::Warn);
    }
}

pub(super) fn trace_frames() {
    log::set_max_level(log::LevelFilter::Debug);
}
