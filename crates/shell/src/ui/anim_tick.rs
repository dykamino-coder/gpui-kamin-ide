//! Общий такт нативных анимаций: кадр — только к смене состояния (INC-2026-0001).
//!
//! `with_animation` заказывает кадр на каждую развёртку, поэтому пульс точки
//! статуса держал окно на 60 кадрах всё время, пока Claude работает, а мигание
//! курсора терминала — пока терминал на экране. Здесь анимация дискретна:
//! рендер спрашивает номер шага и тем самым просит кадр к началу следующего
//! шага; один поток будит окно к ближайшему такому сроку и спит, когда просьб
//! нет. Шаги считаются от общих часов, поэтому анимации с кратными шагами
//! меняются в одном кадре.
//!
//! Два адресата срока (INC-2026-0064). `step` просит перерисовать корень окна:
//! раскладка всего окна. `step_for_view` просит перерисовать один вью: насос
//! уведомляет только его, и изолированный вью (`ui::ticking`) перерисовывается
//! без корня и соседей.

use std::sync::{Mutex, OnceLock};
use std::thread::Thread;
use std::time::{Duration, Instant};

/// Сроки (мс от `EPOCH`), к которым нужен кадр.
struct Due {
    /// Корню окна; `u64::MAX` — не нужен.
    root: u64,
    /// Вью и срок каждого.
    views: Vec<(gpui::EntityId, u64)>,
    /// Вью, чей срок наступил: их забирает насос.
    ready: Vec<gpui::EntityId>,
}

static DUE: Mutex<Due> = Mutex::new(Due {
    root: u64::MAX,
    views: Vec::new(),
    ready: Vec::new(),
});
static EPOCH: OnceLock<Instant> = OnceLock::new();
static TICKER: OnceLock<Thread> = OnceLock::new();

fn now_ms() -> u64 {
    EPOCH.get_or_init(Instant::now).elapsed().as_millis() as u64
}

fn due() -> std::sync::MutexGuard<'static, Due> {
    DUE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Номер текущего шага длиной `step_ms` и начало следующего.
fn next_step(step_ms: u64) -> (u64, u64) {
    let step_ms = step_ms.max(1);
    let now = now_ms();
    (now / step_ms, (now / step_ms + 1) * step_ms)
}

/// Номер текущего шага длиной `step_ms` по общим часам. Вызов из рендера
/// просит кадр корня к началу следующего шага: перестала анимация
/// рисоваться — кадров больше нет.
pub(crate) fn step(step_ms: u64) -> u64 {
    let (n, at) = next_step(step_ms);
    repaint_at(at);
    n
}

/// Вью `id` рисует ступенчатую анимацию: просит перерисовку только себя к
/// началу следующего шага `step_ms`. Подписка одноразовая — пока анимация
/// нужна, вью повторяет вызов из `render`.
pub(crate) fn step_for_view(id: gpui::EntityId, step_ms: u64) -> u64 {
    let (n, at) = next_step(step_ms);
    let mut due = due();
    match due.views.iter_mut().find(|(view, _)| *view == id) {
        Some((_, when)) => *when = (*when).min(at),
        None => due.views.push((id, at)),
    }
    let earliest = earliest(&due);
    drop(due);
    if earliest == at {
        ticker().unpark();
    }
    n
}

/// Кадр корня к моменту `at` общих часов (`now`).
pub(crate) fn repaint_at(at: u64) {
    let mut due = due();
    if at < due.root {
        due.root = at;
        let earliest = earliest(&due);
        drop(due);
        if earliest == at {
            ticker().unpark();
        }
    }
}

/// Забрать вью, чей срок наступил (зовёт насос и уведомляет каждый).
pub(crate) fn take_view_subscribers() -> Vec<gpui::EntityId> {
    std::mem::take(&mut due().ready)
}

/// Миллисекунды общих часов (для «сколько прошло» без нового кадра).
pub(crate) fn now() -> u64 {
    now_ms()
}

fn earliest(due: &Due) -> u64 {
    due.views.iter().map(|(_, at)| *at).fold(due.root, u64::min)
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

/// Что наступило к `now`: корень и/или вью (вью переходят в `ready`).
fn fire(due: &mut Due, now: u64) -> (bool, bool) {
    let root = due.root <= now;
    if root {
        due.root = u64::MAX;
    }
    let before = due.ready.len();
    let Due { views, ready, .. } = due;
    views.retain(|(id, at)| {
        let fired = *at <= now;
        if fired && !ready.contains(id) {
            ready.push(*id);
        }
        !fired
    });
    (root, due.ready.len() > before)
}

fn run() {
    loop {
        let now = now_ms();
        let mut due = due();
        let earliest = earliest(&due);
        if earliest == u64::MAX {
            drop(due);
            std::thread::park();
            continue;
        }
        if now < earliest {
            drop(due);
            // Более ранний срок разбудит поток через `unpark`.
            std::thread::park_timeout(Duration::from_millis(earliest - now));
            continue;
        }
        let (root, views) = fire(&mut due, now);
        drop(due);
        if root {
            crate::web::diag_why::tick("animation/tick");
            crate::web::repaint_requested();
        } else if views {
            crate::web::diag_why::tick("animation/view-tick");
            crate::web::wake_pump();
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
    use super::{Due, elapsed_label, fire};

    #[test]
    fn elapsed_label_grows_from_seconds_to_hours() {
        assert_eq!(elapsed_label(0), "0s");
        assert_eq!(elapsed_label(59_999), "59s");
        assert_eq!(elapsed_label(65_000), "1:05");
        assert_eq!(elapsed_label(3_725_000), "1:02:05");
    }

    #[test]
    fn a_view_deadline_wakes_only_its_view() {
        let a = gpui::EntityId::from(9_001u64);
        let b = gpui::EntityId::from(9_002u64);
        let mut due = Due {
            root: u64::MAX,
            views: vec![(a, 100), (b, 1_000)],
            ready: Vec::new(),
        };
        assert_eq!(
            fire(&mut due, 100),
            (false, true),
            "the root is not redrawn"
        );
        assert_eq!(due.ready, vec![a]);
        assert_eq!(due.views, vec![(b, 1_000)], "a later deadline waits");
        due.root = 900;
        assert_eq!(fire(&mut due, 1_000), (true, true));
        assert_eq!(due.ready, vec![a, b]);
        assert!(due.views.is_empty());
    }
}
