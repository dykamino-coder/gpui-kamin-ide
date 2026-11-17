//! Остановка CEF при выходе (INC-2026-0122).
//!
//! `CefShutdown` при живых браузерах сам закрывает их и ждёт их процессы:
//! выход тянулся от 0,3 до 4,4 с. Поэтому браузеры закрываются заранее, выход
//! ждёт `on_before_close` последнего с пределом, а штампы строк `diag.log`
//! отделяют выход цикла окон, закрытие браузеров и сам `CefShutdown`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// Браузеры, которые CEF создал и ещё не закрыл.
static OPEN: AtomicUsize = AtomicUsize::new(0);

/// Сколько выход ждёт закрытия браузеров, прежде чем отдать их `CefShutdown`.
const CLOSE_LIMIT: Duration = Duration::from_secs(2);

/// CEF создал браузер (`on_after_created`).
pub(in crate::web) fn opened() {
    OPEN.fetch_add(1, Ordering::SeqCst);
}

/// CEF закрыл браузер (`on_before_close`).
pub(in crate::web) fn closed() {
    let _ = OPEN.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1));
}

/// Закрыть браузеры, дождаться их и остановить CEF.
pub(super) fn stop_cef() {
    let log = crate::web::startup_log;
    let open = OPEN.load(Ordering::SeqCst);
    log(format!(
        "[exit] window loop finished, closing {open} browser(s)"
    ));
    let started = Instant::now();
    for id in crate::web::browsers::ids() {
        crate::web::browsers::close(&id);
    }
    let left = wait_until_zero(&OPEN, CLOSE_LIMIT);
    log(format!(
        "[exit] browsers closed in {} ms, {left} left open",
        started.elapsed().as_millis()
    ));
    let started = Instant::now();
    cef::shutdown();
    log(format!(
        "[exit] CEF stopped in {} ms",
        started.elapsed().as_millis()
    ));
    crate::web::flush_startup_log(Duration::from_millis(500));
}

/// Ждать, пока счёт не станет нулём, не дольше `limit`; вернуть остаток.
fn wait_until_zero(count: &AtomicUsize, limit: Duration) -> usize {
    let started = Instant::now();
    loop {
        let open = count.load(Ordering::SeqCst);
        if open == 0 || started.elapsed() >= limit {
            return open;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn waits_for_the_last_close_and_reports_what_stays_open() {
        let count = Arc::new(AtomicUsize::new(2));
        let closer = count.clone();
        let worker = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            closer.fetch_sub(1, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(30));
            closer.fetch_sub(1, Ordering::SeqCst);
        });
        assert_eq!(wait_until_zero(&count, Duration::from_secs(5)), 0);
        worker.join().unwrap();
        count.store(1, Ordering::SeqCst);
        let started = Instant::now();
        assert_eq!(wait_until_zero(&count, Duration::from_millis(40)), 1);
        assert!(started.elapsed() >= Duration::from_millis(40));
    }
}
