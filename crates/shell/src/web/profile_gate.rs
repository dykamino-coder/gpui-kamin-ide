//! Решения старта CEF при занятом профиле (INC-2026-0076): короткое ожидание
//! перед `initialize`, классификация отказа, автоматический перезапуск и
//! аргументы преемника. Только чистые функции; ввод-вывод — в
//! `profile_probe.rs`, `availability.rs` и `relaunch.rs`.
//!
//! Почему перезапуск, а не повтор: `cef_app.h:68-76` запрещает после неудачного
//! `CefInitialize` звать CEF в том же процессе (кроме `CefGetExitCode`).

use std::time::Duration;

/// Предел ожидания до окна: дольше — пустой экран без объяснений. Хвост
/// прошлого инстанса живёт до 40 с, его дожидается автоматика уже при окне.
pub(crate) const PRE_INIT_BUDGET: Duration = Duration::from_secs(5);
const STEP_MIN: Duration = Duration::from_millis(100);
const STEP_MAX: Duration = Duration::from_secs(1);
/// Автоперезапусков подряд не больше: ложное «свободно» не должно зациклить
/// приложение. Кнопка пользователя счёт сбрасывает.
pub(crate) const MAX_AUTO_RELAUNCHES: u32 = 2;

/// `cef_types.h:1106` `CEF_RESULT_CODE_PROFILE_IN_USE`.
pub(crate) const PROFILE_IN_USE: i32 = 21;
/// `cef_types.h:1113` `CEF_RESULT_CODE_NORMAL_EXIT_PROCESS_NOTIFIED`.
pub(crate) const PROCESS_NOTIFIED: i32 = 24;

/// Аргумент преемника: PID процесса, чьего выхода ждать до single-instance.
pub(crate) const AFTER_FLAG: &str = "--kamin-relaunch-after=";
/// Аргумент преемника: сколько автоперезапусков уже было подряд.
pub(crate) const COUNT_FLAG: &str = "--kamin-relaunch-count=";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Gate {
    Init,
    Wait(Duration),
}

/// Шаг ожидания перед `initialize`: свободно или бюджет исчерпан — звать CEF
/// (окончательно решает он сам); иначе растущая пауза, не дальше бюджета.
pub(crate) fn pre_init_step(busy: bool, waited: Duration) -> Gate {
    if !busy || waited >= PRE_INIT_BUDGET {
        return Gate::Init;
    }
    let step = (waited / 2).clamp(STEP_MIN, STEP_MAX);
    Gate::Wait(step.min(PRE_INIT_BUDGET - waited))
}

/// Итог `initialize` с кодом `get_exit_code()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Live,
    /// Профиль держит другой процесс KaminIDE (21 или 24).
    ProfileBusy(i32),
    /// Иной отказ: перезапуск сам по себе не поможет.
    Failed(i32),
}

pub(crate) fn classify(ok: i32, exit_code: i32) -> Outcome {
    if ok == 1 {
        Outcome::Live
    } else if matches!(exit_code, PROFILE_IN_USE | PROCESS_NOTIFIED) {
        Outcome::ProfileBusy(exit_code)
    } else {
        Outcome::Failed(exit_code)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Recovery {
    /// Ничего не делать: профиль ещё занят.
    Wait,
    /// Запустить преемника и штатно закрыть окно.
    Relaunch,
    /// Решает пользователь: несохранённые правки, исчерпан лимит или отказ
    /// не про занятый профиль.
    AskUser,
}

/// Что делать с окном без веб-содержимого. `manual` — кнопка Restart;
/// иначе сигнал фонового наблюдателя «профиль освободился».
pub(crate) fn recovery_step(
    outcome: Outcome,
    profile_free: bool,
    unsaved: bool,
    relaunches: u32,
    manual: bool,
) -> Recovery {
    if unsaved {
        return Recovery::AskUser;
    }
    if manual {
        return Recovery::Relaunch;
    }
    match outcome {
        Outcome::ProfileBusy(_) if !profile_free => Recovery::Wait,
        Outcome::ProfileBusy(_) if relaunches < MAX_AUTO_RELAUNCHES => Recovery::Relaunch,
        Outcome::Live => Recovery::Wait,
        _ => Recovery::AskUser,
    }
}

/// Числовое значение `flag` из аргументов процесса.
pub(crate) fn flag_value(args: &[String], flag: &str) -> Option<u32> {
    args.iter()
        .find_map(|a| a.strip_prefix(flag))
        .and_then(|v| v.parse().ok())
}

/// Аргументы преемника: исходные (папка «Open with», ключи стенда) без
/// прежних наших флагов, плюс PID предшественника и счёт перезапусков.
pub(crate) fn successor_args(args: &[String], pid: u32, relaunches: u32) -> Vec<String> {
    args.iter()
        .filter(|a| !a.starts_with(AFTER_FLAG) && !a.starts_with(COUNT_FLAG))
        .cloned()
        .chain([
            format!("{AFTER_FLAG}{pid}"),
            format!("{COUNT_FLAG}{relaunches}"),
        ])
        .collect()
}

#[cfg(test)]
#[path = "profile_gate_tests.rs"]
mod tests;
