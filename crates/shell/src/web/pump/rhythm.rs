//! Общий ритм фоновых перерисовок окна (INC-2026-0001).
//!
//! gpui не перерисовывает часть окна: каждый кадр — всё окно. Фоновые
//! источники — кадры веб-панелей (чат, консоль), события хоста и такт нативных
//! анимаций — заказывали кадры каждый в своё время, и окно рисовало их сумму:
//! ~50 кадров в секунду, пока Claude отвечает, хотя ни один источник столько
//! не просил. Здесь насос выпускает фоновый кадр только на границе общей сетки
//! (`SLOT_MS`, ~30 в секунду; при программном рендере — его 100 мс): всё, что
//! накопилось за слот, уходит в один кадр. Ввод пользователя ритм обходит —
//! пока он был недавно, кадр уходит сразу: прокрутка страницы, ховеры и ответ
//! на клик не ждут слота.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Слот общей сетки на видеокарте: ~30 кадров в секунду.
const SLOT_MS: u64 = 33;
/// Слот без видеокарты — тот же темп, что и пауза насоса (`gpu_mode`).
const SOFTWARE_SLOT_MS: u64 = 100;
/// Сколько после ввода кадры идут без ожидания слота.
const INPUT_GRACE_MS: u64 = 300;

static EPOCH: OnceLock<Instant> = OnceLock::new();
/// Момент последнего ввода (мс от `EPOCH` + 1; 0 — ввода ещё не было).
static LAST_INPUT: AtomicU64 = AtomicU64::new(0);

fn now_ms() -> u64 {
    EPOCH.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// Ввод пользователя: в `[why]` под именем `name`, и ближайшие кадры — сразу.
pub(crate) fn input(name: &'static str) {
    crate::web::diag_why::tick(name);
    LAST_INPUT.store(now_ms() + 1, Ordering::Relaxed);
}

/// Сколько насосу ждать до слота, в котором можно выпустить фоновый кадр.
pub(crate) fn delay(software: bool) -> Duration {
    let slot = if software { SOFTWARE_SLOT_MS } else { SLOT_MS };
    Duration::from_millis(delay_ms(now_ms(), LAST_INPUT.load(Ordering::Relaxed), slot))
}

fn delay_ms(now: u64, last_input: u64, slot: u64) -> u64 {
    if last_input != 0 && now.saturating_sub(last_input - 1) < INPUT_GRACE_MS {
        return 0;
    }
    now.div_ceil(slot) * slot - now
}

#[cfg(test)]
mod tests {
    use super::{INPUT_GRACE_MS, SLOT_MS, delay_ms};

    #[test]
    fn background_frames_wait_for_the_next_slot_boundary() {
        assert_eq!(delay_ms(SLOT_MS, 0, SLOT_MS), 0);
        assert_eq!(delay_ms(SLOT_MS + 1, 0, SLOT_MS), SLOT_MS - 1);
        assert_eq!(delay_ms(10 * SLOT_MS - 5, 0, SLOT_MS), 5);
    }

    #[test]
    fn recent_input_skips_the_wait_and_old_input_does_not() {
        let at = 1000;
        assert_eq!(delay_ms(at + 10, at + 1, SLOT_MS), 0);
        assert!(delay_ms(at + INPUT_GRACE_MS + 1, at + 1, SLOT_MS) > 0);
    }
}
