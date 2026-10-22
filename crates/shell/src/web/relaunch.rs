//! Перезапуск ради веб-содержимого: CEF не инициализируется дважды в одном
//! процессе, поэтому окно без CEF восстанавливается новым процессом.
//!
//! Предшественник запускает преемника и штатно закрывает окно (сохранение
//! раскладки, `layout_store::flush_now`); преемник ждёт его выхода ДО проверки
//! single-instance — иначе отдал бы фокус уходящему окну и вышел сам.

use std::time::Duration;

use super::profile_gate::{AFTER_FLAG, COUNT_FLAG, flag_value, successor_args};

/// Сколько преемник ждёт предшественника: штатный выход без CEF занимает
/// доли секунды; предел защищает от зависшего или переиспользованного PID.
const PREDECESSOR_WAIT: Duration = Duration::from_secs(20);

fn args() -> Vec<String> {
    std::env::args().skip(1).collect()
}

/// Автоперезапусков подряд до этого процесса.
pub(crate) fn relaunch_count() -> u32 {
    flag_value(&args(), COUNT_FLAG).unwrap_or(0)
}

/// Преемник: дождаться выхода процесса, который нас запустил.
pub fn wait_for_predecessor() {
    if let Some(pid) = flag_value(&args(), AFTER_FLAG) {
        super::profile_probe::wait_for_exit(pid, PREDECESSOR_WAIT);
    }
}

/// Запустить преемника вне нашего Job (`job.rs`: KILL_ON_JOB_CLOSE убил бы его
/// вместе с нами). `auto` — автоматика, иначе кнопка (счёт сбрасывается).
#[cfg(windows)]
pub(crate) fn spawn_successor(auto: bool) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt as _;
    const DETACHED_PROCESS: u32 = 0x8;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x200;
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
    let count = if auto { relaunch_count() + 1 } else { 0 };
    let exe = std::env::current_exe()?;
    std::process::Command::new(exe)
        .args(successor_args(&args(), std::process::id(), count))
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB)
        .spawn()?;
    super::diag_sink::emit_line(format!("[cef] relaunch spawned auto={auto} count={count}"));
    Ok(())
}

#[cfg(not(windows))]
pub(crate) fn spawn_successor(_auto: bool) -> std::io::Result<()> {
    Err(std::io::Error::other(
        "relaunch is implemented for Windows only",
    ))
}
