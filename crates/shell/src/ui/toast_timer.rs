//! Каунтдаун тоста: активное время без ховер-пауз.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// Живой таймер тоста. Живёт в `RootView::toast_timers` (не в самом Toast —
/// у Toast 18 мест конструирования). Wall-clock: полоса каунтдауна читает
/// active_ms() на КАЖДЫЙ кадр (насос — repeat-анимация на полосе), ховер
/// держит паузу, `closing` запускает slide-out; фоновый поток обработчика
/// `ShellEvent::Toast` лишь проверяет истечение раз в 100мс.
pub struct ToastTimer {
    started: std::time::Instant,
    /// Сумма мс ЗАВЕРШЁННЫХ ховер-пауз.
    paused_ms: AtomicU32,
    /// Начало текущего ховера, мс от started; u32::MAX = ховера нет.
    hover_since_ms: AtomicU32,
    pub closing: AtomicBool,
}

impl Default for ToastTimer {
    fn default() -> Self {
        Self {
            started: std::time::Instant::now(),
            paused_ms: AtomicU32::new(0),
            hover_since_ms: AtomicU32::new(u32::MAX),
            closing: AtomicBool::new(false),
        }
    }
}

impl ToastTimer {
    /// Прожитые мс БЕЗ ховер-пауз (гладкая, не ступенчатая величина).
    pub fn active_ms(&self) -> u32 {
        self.active_at(self.started.elapsed().as_millis() as u32)
    }

    fn active_at(&self, now: u32) -> u32 {
        let hover = self.hover_since_ms.load(Ordering::Relaxed);
        let current_pause = if hover == u32::MAX {
            0
        } else {
            now.saturating_sub(hover)
        };
        now.saturating_sub(self.paused_ms.load(Ordering::Relaxed))
            .saturating_sub(current_pause)
    }

    pub fn set_hovered(&self, hovered: bool) {
        self.set_hovered_at(hovered, self.started.elapsed().as_millis() as u32);
    }

    fn set_hovered_at(&self, hovered: bool, now: u32) {
        if hovered {
            let _ = self.hover_since_ms.compare_exchange(
                u32::MAX,
                now,
                Ordering::Relaxed,
                Ordering::Relaxed,
            );
        } else {
            let since = self.hover_since_ms.swap(u32::MAX, Ordering::Relaxed);
            if since != u32::MAX {
                self.paused_ms
                    .fetch_add(now.saturating_sub(since), Ordering::Relaxed);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ховер_останавливает_каунтдаун_и_продолжает_его_после_ухода() {
        let timer = ToastTimer::default();
        assert_eq!(timer.active_at(2000), 2000);
        timer.set_hovered_at(true, 2000);
        timer.set_hovered_at(true, 3000);
        assert_eq!(timer.active_at(12000), 2000);
        timer.set_hovered_at(false, 12000);
        assert_eq!(timer.active_at(15999), 5999);
        assert_eq!(timer.active_at(16000), 6000);
        timer.set_hovered_at(false, 16000);
        assert_eq!(timer.active_at(18000), 8000);
    }

    #[test]
    fn несколько_пауз_вычитаются_из_активного_времени() {
        let timer = ToastTimer::default();
        timer.set_hovered_at(true, 1000);
        timer.set_hovered_at(false, 3000);
        timer.set_hovered_at(true, 4000);
        timer.set_hovered_at(false, 7000);
        assert_eq!(timer.active_at(11000), 6000);
    }
}
