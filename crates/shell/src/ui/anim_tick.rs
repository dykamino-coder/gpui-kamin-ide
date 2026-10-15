//! Общий медленный такт нативных анимаций (INC-2026-0001).
//!
//! gpui не перерисовывает часть окна: любой шаг даже 6-пиксельной анимации —
//! целый кадр окна (~3,8 мс GPU на Intel UHD, ~22 мс на WARP). `with_animation`
//! заказывает кадр на каждую развёртку, поэтому пульс точки статуса держал окно
//! на 60 кадрах всё время, пока Claude работает (замер: 126 тиков/с на две
//! точки). Здесь анимация берёт фазу с часов, округлённых до `STEP_MS`, а кадры
//! заказывает один поток — раз в шаг, пока хоть одна такая анимация была на
//! экране в последние два шага. Все медленные анимации делят этот такт, поэтому
//! сколько бы их ни было, окно перерисовывается не чаще 10 раз в секунду.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Шаг такта: 10 кадров в секунду.
const STEP_MS: u64 = 100;

static EPOCH: OnceLock<Instant> = OnceLock::new();
/// До какого момента (мс от `EPOCH`) кому-то нужен такт.
static WANTED_UNTIL: AtomicU64 = AtomicU64::new(0);
/// Поток такта жив.
static RUNNING: AtomicBool = AtomicBool::new(false);

fn now_ms() -> u64 {
    EPOCH.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// Фаза `0..1` периода `period_ms` по общему такту. Вызов из рендера держит
/// такт живым ещё два шага: исчезла анимация с экрана — такт затих сам.
pub(crate) fn phase(period_ms: u64) -> f32 {
    let now = now_ms();
    WANTED_UNTIL.fetch_max(now + 2 * STEP_MS, Ordering::Relaxed);
    ensure_ticker();
    step_phase(now, period_ms)
}

/// Фаза момента `now`, округлённого вниз до шага такта.
fn step_phase(now: u64, period_ms: u64) -> f32 {
    let period = period_ms.max(1);
    let stepped = now / STEP_MS * STEP_MS;
    (stepped % period) as f32 / period as f32
}

fn ensure_ticker() {
    if RUNNING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    std::thread::spawn(|| {
        loop {
            let now = now_ms();
            let next = (now / STEP_MS + 1) * STEP_MS;
            std::thread::sleep(Duration::from_millis(next - now));
            if now_ms() > WANTED_UNTIL.load(Ordering::Relaxed) {
                RUNNING.store(false, Ordering::Release);
                // Анимация могла попросить такт между проверкой и сбросом
                // флага и не запустить поток (он ещё числился живым).
                let wanted = now_ms() <= WANTED_UNTIL.load(Ordering::Relaxed);
                if !wanted
                    || RUNNING
                        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                        .is_err()
                {
                    return;
                }
            }
            crate::web::diag_why::tick("animation/tick");
            crate::web::repaint_requested();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::step_phase;

    #[test]
    fn phase_moves_only_on_step_boundaries() {
        assert_eq!(step_phase(0, 1100), 0.0);
        assert_eq!(step_phase(99, 1100), 0.0);
        assert_eq!(step_phase(100, 1100), step_phase(199, 1100));
        assert!(step_phase(200, 1100) > step_phase(100, 1100));
    }

    #[test]
    fn phase_wraps_with_the_period() {
        assert_eq!(step_phase(1100, 1100), 0.0);
        assert_eq!(step_phase(1200, 1100), step_phase(100, 1100));
    }
}
