//! Корреляция native-записей по UTC epoch, PID и времени запуска без private paths.

use std::sync::{LazyLock, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use super::diag_log::Log;

struct Sink {
    log: Log,
    boot_written: bool,
    run_ms: u128,
    since: Instant,
}

pub(super) fn emit_line(line: String) {
    static SINK: LazyLock<Mutex<Sink>> = LazyLock::new(|| {
        Mutex::new(Sink {
            log: Log::new(
                crate::host::paths::data_dirs().1.join("diag.log"),
                5 * 1024 * 1024,
            ),
            boot_written: false,
            run_ms: epoch_ms(),
            since: Instant::now(),
        })
    });
    let Ok(mut sink) = SINK.lock() else { return };
    let stamp = format!(
        "utc_ms={} pid={} run_ms={} elapsed_ms={}",
        epoch_ms(),
        std::process::id(),
        sink.run_ms,
        sink.since.elapsed().as_millis()
    );
    if !sink.boot_written {
        sink.boot_written = sink
            .log
            .append(&format!(
                "[{stamp}] [boot] KaminIDE {}",
                env!("CARGO_PKG_VERSION")
            ))
            .is_ok();
    }
    let record = format!("[{stamp}] {line}");
    println!("{record}");
    // Ошибка файловой системы не отключает диагностику навсегда: следующий
    // heartbeat повторит запись. В stdout остаётся сигнал без private path.
    if sink.log.append(&record).is_err() {
        eprintln!("[diag] diagnostic record could not be persisted");
    }
}

fn epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
