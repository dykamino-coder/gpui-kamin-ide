//! KaminIDE patch: возврат освобождённой памяти, когда кадры стихли.
//!
//! Вход: конец каждого нарисованного кадра любого окна ([`IdleReclaim::note_draw`])
//! и каждый WM_PAINT, который vsync присылает и тогда, когда рисовать нечего
//! ([`IdleReclaim::poll`]). Выход: один вызов действия, которое зарегистрировало
//! приложение ([`set_idle_reclaim`]), после того как ни одно окно не рисовало
//! заданную паузу. Инвариант: не больше одного вызова на серию кадров; пока идёт
//! анимация (кадр каждый vsync) или ввод чаще паузы, вызова нет.
//!
//! Платформа не знает, каким аллокатором собрано приложение, поэтому действие
//! даёт приложение. Так же Chromium отдаёт свободные страницы PartitionAlloc
//! по таймеру простоя, а не на каждом освобождении.

use std::{
    cell::Cell,
    sync::OnceLock,
    time::{Duration, Instant},
};

struct Policy {
    quiet: Duration,
    reclaim: fn(),
}

static POLICY: OnceLock<Policy> = OnceLock::new();

/// Регистрирует действие простоя процесса: `reclaim` вызывается на UI-потоке,
/// когда после последнего кадра прошло `quiet` без новых кадров. Действует
/// первая регистрация; без неё платформа ничего не вызывает.
pub fn set_idle_reclaim(quiet: Duration, reclaim: fn()) {
    let _ = POLICY.set(Policy { quiet, reclaim });
}

/// Общий для всех окон учёт серии кадров (живёт в `DrawCoordinator`).
pub(crate) struct IdleReclaim {
    last_draw: Cell<Option<Instant>>,
}

impl IdleReclaim {
    pub(crate) fn new() -> Self {
        Self {
            last_draw: Cell::new(None),
        }
    }

    /// Кадр нарисован: серия продолжается, отсчёт паузы начинается заново.
    pub(crate) fn note_draw(&self) {
        if POLICY.get().is_some() {
            self.last_draw.set(Some(Instant::now()));
        }
    }

    /// Вызывает действие простоя, если серия кадров закончилась паузой.
    pub(crate) fn poll(&self) {
        let Some(policy) = POLICY.get() else {
            return;
        };
        if self.last_draw.get().is_some() && self.due(policy.quiet, Instant::now()) {
            (policy.reclaim)();
        }
    }

    /// Пауза после последнего кадра достигла `quiet`: серия закрывается.
    fn due(&self, quiet: Duration, now: Instant) -> bool {
        match self.last_draw.get() {
            Some(at) if now.saturating_duration_since(at) >= quiet => {
                self.last_draw.set(None);
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const QUIET: Duration = Duration::from_millis(250);

    fn drawn_at(at: Instant) -> IdleReclaim {
        let idle = IdleReclaim::new();
        idle.last_draw.set(Some(at));
        idle
    }

    #[test]
    fn nothing_is_due_before_the_first_frame() {
        let idle = IdleReclaim::new();
        assert!(!idle.due(QUIET, Instant::now() + QUIET * 4));
    }

    #[test]
    fn a_burst_is_reclaimed_once_after_the_quiet_pause() {
        let start = Instant::now();
        let idle = drawn_at(start);
        assert!(!idle.due(QUIET, start + QUIET / 2));
        assert!(idle.due(QUIET, start + QUIET));
        assert!(!idle.due(QUIET, start + QUIET * 8));
    }

    #[test]
    fn frames_closer_than_the_pause_keep_the_burst_open() {
        let start = Instant::now();
        let idle = drawn_at(start);
        let mut now = start;
        for _ in 0..40 {
            now += Duration::from_millis(16);
            assert!(!idle.due(QUIET, now));
            idle.last_draw.set(Some(now));
        }
        assert!(idle.due(QUIET, now + QUIET));
    }
}
