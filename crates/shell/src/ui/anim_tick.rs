//! Общий такт нативных анимаций: кадр — только к смене состояния (INC-2026-0001).
//!
//! gpui не перерисовывает часть окна: любой шаг даже 6-пиксельной анимации —
//! целый кадр окна (~3,8 мс GPU на Intel UHD, ~22 мс на WARP). `with_animation`
//! заказывает кадр на каждую развёртку, поэтому пульс точки статуса держал окно
//! на 60 кадрах всё время, пока Claude работает, а мигание курсора терминала —
//! пока терминал на экране. Здесь анимация дискретна: рендер спрашивает номер
//! шага (`step`) и тем самым просит кадр к началу следующего шага; один поток
//! будит окно к ближайшему такому сроку и спит, когда просьб нет. Шаги считаются
//! от общих часов, поэтому анимации с кратными шагами меняются в одном кадре.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::Thread;
use std::time::{Duration, Instant};

/// Ближайший срок, к которому кому-то нужен кадр (мс от `EPOCH`).
static NEXT_DUE: AtomicU64 = AtomicU64::new(u64::MAX);
static EPOCH: OnceLock<Instant> = OnceLock::new();
static TICKER: OnceLock<Thread> = OnceLock::new();

fn now_ms() -> u64 {
    EPOCH.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// Номер текущего шага длиной `step_ms` по общим часам. Вызов из рендера
/// просит кадр к началу следующего шага: перестала анимация рисоваться —
/// кадров больше нет.
pub(crate) fn step(step_ms: u64) -> u64 {
    let step_ms = step_ms.max(1);
    let now = now_ms();
    want_at((now / step_ms + 1) * step_ms);
    now / step_ms
}

/// Миллисекунды общих часов (для «сколько прошло» без нового кадра).
pub(crate) fn now() -> u64 {
    now_ms()
}

fn want_at(due: u64) {
    let prev = NEXT_DUE.fetch_min(due, Ordering::AcqRel);
    if due < prev {
        ticker().unpark();
    }
}

fn ticker() -> &'static Thread {
    TICKER.get_or_init(|| {
        std::thread::Builder::new()
            .name("anim-tick".into())
            .spawn(run)
            .expect("anim-tick thread")
            .thread()
            .clone()
    })
}

fn run() {
    loop {
        let due = NEXT_DUE.load(Ordering::Acquire);
        if due == u64::MAX {
            std::thread::park();
            continue;
        }
        let now = now_ms();
        if now < due {
            // Более ранний срок разбудит поток через `unpark`.
            std::thread::park_timeout(Duration::from_millis(due - now));
            continue;
        }
        if NEXT_DUE
            .compare_exchange(due, u64::MAX, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            crate::web::diag_why::tick("animation/tick");
            crate::web::repaint_requested();
        }
    }
}

/// Прошедшее время `ms` для подписи: `9s`, `1:05`, `1:02:05`.
pub(crate) fn elapsed_label(ms: u64) -> String {
    let s = ms / 1000;
    match (s / 3600, s / 60 % 60, s % 60) {
        (0, 0, sec) => format!("{sec}s"),
        (0, min, sec) => format!("{min}:{sec:02}"),
        (h, min, sec) => format!("{h}:{min:02}:{sec:02}"),
    }
}

#[cfg(test)]
mod tests {
    use super::elapsed_label;

    #[test]
    fn elapsed_label_grows_from_seconds_to_hours() {
        assert_eq!(elapsed_label(0), "0s");
        assert_eq!(elapsed_label(59_999), "59s");
        assert_eq!(elapsed_label(65_000), "1:05");
        assert_eq!(elapsed_label(3_725_000), "1:02:05");
    }
}
