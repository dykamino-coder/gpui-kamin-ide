//! Ватчдог профиля CEF: вью без кадров и вью с пустыми кадрами.
//!
//! Браузер, у которого видимое вью 12 с без единого кадра, закрывается — его
//! поднимет следующая отрисовка (`ensure_html_view`). Это лечит редкие простои
//! CEF на старте; если кадров нет, вероятно, нет и GPU-композитинга (RDP),
//! поэтому процесс переходит в software-профиль.
//!
//! Переход ОБЩИЙ: `sw_mode()` влияет только на новые браузеры, поэтому вместе
//! с «молчащими» пересоздаются ВСЕ живые браузеры на общих текстурах, включая
//! скрытые и никогда не выгружаемый чат. Раньше пересоздавались только
//! молчащие, и чат с консолью оставались на общих текстурах, рисуя чёрный
//! кадр до перезапуска (INC-2026-0073). Пустой accelerated-кадр
//! (`blank_probe`) — такой же повод перейти в software, как и тишина.
//!
//! Циклов нет: миграция берёт только браузеры на общих текстурах, а
//! пересозданный уже software; пустые кадры бывают только у accelerated;
//! простой — не больше [`STALL_ATTEMPTS`] раз на вью.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

/// Сколько секунд видимое вью может быть без кадров.
const STALL_SECS: u64 = 12;

/// Попыток пересоздать вью из-за простоя: дальше причина не в браузере.
const STALL_ATTEMPTS: u32 = 2;

/// Снимок одного живого браузера для решения ватчдога.
#[derive(Clone, Debug)]
pub(super) struct ViewProbe {
    pub(super) id: String,
    pub(super) visible: bool,
    /// Браузер создан с общими текстурами.
    pub(super) accelerated: bool,
    /// Есть кадр хоть какого-то пути (общая текстура или software).
    pub(super) has_frame: bool,
    /// Давность заказа браузера.
    pub(super) opened_secs: Option<u64>,
    /// Accelerated-кадры подряд читаются пустыми (`blank_probe`).
    pub(super) blank: bool,
    /// Сколько раз вью уже пересоздавали из-за простоя.
    pub(super) stall_attempts: u32,
}

/// Почему вью пересоздаётся.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Reason {
    /// Видимое вью без кадров дольше [`STALL_SECS`].
    Stalled,
    /// Accelerated-кадры приходят, но читаются пустыми.
    Blank,
    /// Процесс в software, а браузер ещё на общих текстурах.
    Migrate,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Plan {
    /// Включить software-профиль для новых браузеров.
    pub(super) switch_to_software: bool,
    pub(super) respawn: Vec<(String, Reason)>,
}

/// Чистое решение ватчдога: что пересоздать и переходить ли в software.
pub(super) fn plan(views: &[ViewProbe], software: bool) -> Plan {
    let trigger = |v: &ViewProbe| {
        if v.accelerated && v.blank {
            Some(Reason::Blank)
        } else if v.visible
            && !v.has_frame
            && v.opened_secs.is_some_and(|s| s >= STALL_SECS)
            && v.stall_attempts < STALL_ATTEMPTS
        {
            Some(Reason::Stalled)
        } else {
            None
        }
    };
    let triggered = views.iter().any(|v| trigger(v).is_some());
    let software_after = software || triggered;
    let respawn = views
        .iter()
        .filter_map(|v| match trigger(v) {
            Some(reason) => Some((v.id.clone(), reason)),
            None if software_after && v.accelerated => Some((v.id.clone(), Reason::Migrate)),
            None => None,
        })
        .collect();
    Plan {
        switch_to_software: !software && triggered,
        respawn,
    }
}

static STALLS: LazyLock<Mutex<HashMap<String, u32>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Тик ватчдога (раз в секунду из насоса).
pub(crate) fn tick() {
    let visible = super::visibility::visible_set();
    let attempts = STALLS.lock().map(|m| m.clone()).unwrap_or_default();
    let views: Vec<ViewProbe> = super::browsers::ids()
        .into_iter()
        .map(|id| ViewProbe {
            visible: visible.contains(&id),
            accelerated: super::profile::accelerated(&id),
            has_frame: !forced_stall(&id)
                && (super::shared_texture::get(&id).is_some() || super::frames::get(&id).is_some()),
            opened_secs: super::browsers::opened_ago(&id).map(|d| d.as_secs()),
            blank: super::blank_probe::is_blank(&id),
            stall_attempts: attempts.get(&id).copied().unwrap_or(0),
            id,
        })
        .collect();
    let plan = plan(&views, super::sw_mode());
    if plan.switch_to_software {
        super::set_sw_mode(true);
    }
    for (id, reason) in &plan.respawn {
        if *reason == Reason::Stalled
            && let Ok(mut m) = STALLS.lock()
        {
            *m.entry(id.clone()).or_insert(0) += 1;
        }
        super::diag_sink::emit_line(format!(
            "[cef] вью {id}: пересоздаю в software ({reason:?})"
        ));
        respawn(id);
    }
    if !plan.respawn.is_empty() {
        super::repaint_requested();
    }
}

/// Закрыть браузер вью и забыть его кадры — тем же путём, что выгрузка
/// (`reap_hidden`). Возврат вью поднимет браузер заново, а exthost получит
/// «вью видимо» и отдаст странице состояние, как при первом открытии.
fn respawn(id: &str) {
    super::visibility::notify_view_state(id, false);
    super::browsers::close(id);
    super::shared_texture::forget_view(id);
    super::copy_frame::forget_view(id);
    super::frames::forget_view(id);
    super::popup::forget_view(id);
    super::blank_probe::forget_view(id);
}

/// Стенд (только debug): `KAMIN_DEBUG_FORCE_STALL=<id>` объявляет вью
/// молчащим, пока процесс не в software, — так воспроизводится переход
/// ватчдога на машине, где все кадры приходят.
fn forced_stall(id: &str) -> bool {
    #[cfg(debug_assertions)]
    {
        !super::sw_mode()
            && std::env::var("KAMIN_DEBUG_FORCE_STALL").is_ok_and(|v| v.split(',').any(|s| s == id))
    }
    #[cfg(not(debug_assertions))]
    {
        let _ = id;
        false
    }
}

#[cfg(test)]
#[path = "respawn_tests.rs"]
mod tests;
