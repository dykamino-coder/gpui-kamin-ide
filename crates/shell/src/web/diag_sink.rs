//! Корреляция native-записей по UTC epoch, PID и времени запуска без private paths.
//!
//! BR-31: запись в файл идёт на отдельном потоке. `emit_line` зовут с UI-потока
//! (секундная сводка, насос), а открытие, запись и закрытие `diag.log` на этой
//! машине ждали диска до 11 с: всё это время UI-поток не крутил насос, страницы
//! не получали pull, окно не рисовалось. Теперь вызывающий только ставит строку
//! в очередь; штамп времени берётся в момент вызова, как и раньше.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::diag_log::Log;

/// Сколько строк ждут записи, пока писатель стоит на диске. За 11 с простоя
/// сводки набирается ~150 строк; лишнее отбрасывается со счётом.
const QUEUE: usize = 4096;

enum Job {
    Line(String),
    Flush(SyncSender<()>),
}

/// Очередь к потоку-писателю: вызывающий никогда не ждёт файловой системы.
struct Pipe {
    tx: SyncSender<Job>,
    dropped: Arc<AtomicU64>,
}

impl Pipe {
    fn spawn(mut write: impl FnMut(&str) + Send + 'static) -> Self {
        let (tx, rx) = sync_channel::<Job>(QUEUE);
        let dropped = Arc::new(AtomicU64::new(0));
        let lost = dropped.clone();
        let spawned = std::thread::Builder::new()
            .name("kamin-diag-log".into())
            .spawn(move || drain(&rx, &lost, &mut write));
        if spawned.is_err() {
            eprintln!("[diag] log writer thread could not start");
        }
        Self { tx, dropped }
    }

    fn send(&self, record: String) {
        match self.tx.try_send(Job::Line(record)) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
            }
            Err(TrySendError::Disconnected(_)) => {}
        }
    }

    /// Дождаться записи всего, что уже в очереди, не дольше `limit`.
    fn flush(&self, limit: Duration) {
        let deadline = Instant::now() + limit;
        let (done, wait) = sync_channel(1);
        let mut job = Job::Flush(done);
        loop {
            match self.tx.try_send(job) {
                Ok(()) => break,
                Err(TrySendError::Full(back)) if Instant::now() < deadline => {
                    job = back;
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(_) => return,
            }
        }
        let _ = wait.recv_timeout(deadline.saturating_duration_since(Instant::now()));
    }
}

fn drain(rx: &Receiver<Job>, dropped: &AtomicU64, write: &mut impl FnMut(&str)) {
    for job in rx {
        match job {
            Job::Line(record) => {
                let lost = dropped.swap(0, Ordering::Relaxed);
                if lost > 0 {
                    write(&format!(
                        "[diag] dropped {lost} records: the log writer was blocked"
                    ));
                }
                write(&record);
            }
            Job::Flush(done) => {
                let _ = done.send(());
            }
        }
    }
}

struct Stamp {
    run_ms: u128,
    since: Instant,
}

static STAMP: LazyLock<Stamp> = LazyLock::new(|| Stamp {
    run_ms: epoch_ms(),
    since: Instant::now(),
});

static PIPE: LazyLock<Pipe> = LazyLock::new(|| {
    let log = Log::new(
        crate::host::paths::data_dirs().1.join("diag.log"),
        5 * 1024 * 1024,
    );
    let mut boot_written = false;
    Pipe::spawn(move |record| {
        if !boot_written {
            let boot = format!("{} [boot] KaminIDE {}", stamp(), env!("CARGO_PKG_VERSION"));
            boot_written = log.append(&boot).is_ok();
        }
        println!("{record}");
        // Ошибка файловой системы не отключает диагностику навсегда: следующий
        // heartbeat повторит запись. В stdout остаётся сигнал без private path.
        if log.append(record).is_err() {
            eprintln!("[diag] diagnostic record could not be persisted");
        }
    })
});

fn stamp() -> String {
    format!(
        "[utc_ms={} pid={} run_ms={} elapsed_ms={}]",
        epoch_ms(),
        std::process::id(),
        STAMP.run_ms,
        STAMP.since.elapsed().as_millis()
    )
}

pub(crate) fn emit_line(line: String) {
    // Порядок строк сохраняет очередь; мьютекс держит штамп и постановку вместе,
    // чтобы время в файле не шло назад между потоками.
    static ORDER: Mutex<()> = Mutex::new(());
    let _order = ORDER.lock();
    PIPE.send(format!("{} {line}", stamp()));
}

/// Дописать очередь перед выходом процесса мимо обычного завершения.
pub(crate) fn flush(limit: Duration) {
    PIPE.flush(limit);
}

fn epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    #[test]
    fn a_blocked_writer_does_not_block_the_caller() {
        // The writer is stuck as on the BR-31 machine (disk or on-access scan).
        let (release, gate) = channel::<()>();
        let (seen_tx, seen) = channel::<String>();
        let gate = Mutex::new(gate);
        let pipe = Pipe::spawn(move |record| {
            let _ = gate.lock().unwrap().recv();
            let _ = seen_tx.send(record.to_string());
        });
        let started = Instant::now();
        for i in 0..20 {
            pipe.send(format!("line {i}"));
        }
        assert!(
            started.elapsed() < Duration::from_millis(200),
            "emitting waited for the writer: {:?}",
            started.elapsed()
        );
        for _ in 0..20 {
            release.send(()).unwrap();
        }
        let got: Vec<String> = (0..20)
            .map(|_| seen.recv_timeout(Duration::from_secs(5)).unwrap())
            .collect();
        assert_eq!(
            got,
            (0..20).map(|i| format!("line {i}")).collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_full_queue_drops_with_a_count_and_flush_waits_for_the_rest() {
        let (release, gate) = channel::<()>();
        let (seen_tx, seen) = channel::<String>();
        let gate = Mutex::new(Some(gate));
        let pipe = Pipe::spawn(move |record| {
            // Only the first record waits for the gate.
            if let Some(gate) = gate.lock().unwrap().take() {
                let _ = gate.recv();
            }
            let _ = seen_tx.send(record.to_string());
        });
        for i in 0..QUEUE + 10 {
            pipe.send(format!("line {i}"));
        }
        release.send(()).unwrap();
        pipe.flush(Duration::from_secs(10));
        pipe.send("after".into());
        pipe.flush(Duration::from_secs(10));
        let got: Vec<String> = seen.try_iter().collect();
        assert!(
            got.iter().any(|l| l.starts_with("[diag] dropped ")),
            "no drop count"
        );
        assert_eq!(got.last().map(String::as_str), Some("after"));
    }
}
