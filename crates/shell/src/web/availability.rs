//! Состояние «веб-содержимое недоступно» для панелей и автоматика его снятия.
//!
//! До INC-2026-0076 отказ `initialize` был виден только в stderr, а панели
//! навсегда оставались «загружающимися». Теперь итог пишется в `diag.log`,
//! панели показывают явную карточку, а при занятом профиле фоновый
//! наблюдатель ждёт его освобождения и просит окно перезапуститься.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use super::profile_gate::{Outcome, Recovery, recovery_step};

/// Частота проверки профиля: два лёгких системных вызова, кадров не заказывает.
const WATCH_EVERY: Duration = Duration::from_millis(500);

/// Что видят панели, когда CEF в этом процессе не поднялся.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unavailable {
    pub outcome: Outcome,
    /// Профиль уже свободен: перезапуск поможет, но ждёт пользователя.
    pub profile_free: bool,
}

static STATE: Mutex<Option<Unavailable>> = Mutex::new(None);

/// `None` — CEF жив (или ещё не стартовал): панели рисуются как обычно.
pub fn unavailable() -> Option<Unavailable> {
    STATE.lock().ok().and_then(|s| *s)
}

/// Записать отказ `initialize`. При занятом профиле — запустить наблюдателя.
pub(super) fn record_failure(outcome: Outcome, root: Option<PathBuf>) {
    if let Ok(mut s) = STATE.lock() {
        *s = Some(Unavailable {
            outcome,
            profile_free: false,
        });
    }
    if let (Outcome::ProfileBusy(_), Some(root)) = (outcome, root) {
        let spawned = std::thread::Builder::new()
            .name("kamin-cef-profile-watch".into())
            .spawn(move || watch(root));
        if spawned.is_err() {
            mark_free();
        }
    }
}

fn mark_free() {
    if let Ok(mut s) = STATE.lock()
        && let Some(state) = s.as_mut()
    {
        state.profile_free = true;
    }
}

/// Ждать освобождения профиля, затем один раз сообщить окну. Канал событий
/// появляется после `web::init`, поэтому отправку повторяем до его появления.
fn watch(root: PathBuf) {
    while super::profile_probe::busy(&root) {
        std::thread::sleep(WATCH_EVERY);
    }
    mark_free();
    super::diag_sink::emit_line("[cef] profile released by the previous instance".into());
    loop {
        if let Some(tx) = crate::host_link::event_tx() {
            let event = crate::host_link::ShellEvent::Term(
                crate::host::events::TermEvent::RestartForWeb(false),
            );
            let _ = tx.try_send(event);
            return;
        }
        std::thread::sleep(WATCH_EVERY);
    }
}

/// Решение для окна: `manual` — кнопка Restart, иначе сигнал наблюдателя.
pub(crate) fn recovery(unsaved: bool, manual: bool) -> Recovery {
    let Some(state) = unavailable() else {
        return Recovery::Wait;
    };
    recovery_step(
        state.outcome,
        state.profile_free,
        unsaved,
        super::relaunch::relaunch_count(),
        manual,
    )
}
