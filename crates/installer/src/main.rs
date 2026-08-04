//! kaminide-setup — собственный инсталлер KaminIDE (замена NSIS: тот был
//! мишенью AV-эвристик и мигал консолями). Payload (tar.zst каталога
//! dist-installer) приклеен к exe с футером [len:u64 LE]["KMNSETUP"].
//!
//! Режимы:
//!   (без флагов)  установка; GUI-фаза добавит окно, пока — тихая
//!   /S            полностью тихая установка (скрипты, апдейтер)
//!   /update       установка из апдейтера (тихая; после — перезапуск приложения)
//!   /uninstall    удаление
//!
//! Лог: %TEMP%\kaminide-install.log (формат совместим с NSIS-логом).

#![cfg_attr(windows, windows_subsystem = "windows")]

mod payload;
mod steps;

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |f: &str| args.iter().any(|a| a.eq_ignore_ascii_case(f));

    let version = env!("CARGO_PKG_VERSION");
    steps::klog(&format!(
        "onInit exe={} params=[{}]",
        std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default(),
        args.join(" ")
    ));

    if has("/uninstall") {
        if let Err(e) = steps::uninstall() {
            steps::klog(&format!("uninstall FAILED: {e:#}"));
            std::process::exit(1);
        }
        return;
    }

    let install_dir = steps::resolve_install_dir();

    // Побег из Job приложения: апдейтер мог заспавнить нас внутри своего Job
    // (KILL_ON_JOB_CLOSE) — выход приложения убил бы установку на полпути.
    // Наш Job разрешает breakaway (1.0.16+) — пересоздаём себя напрямую,
    // без Планировщика (тот отдавал 0x80070002 на некоторых путях).
    if !has("/KAMINTRAMP") && steps::in_job() {
        match steps::respawn_breakaway(&args) {
            Ok(()) => {
                steps::klog("trampoline: respawned with BREAKAWAY — quitting");
                return; // код 0 — апдейтер считает запуск успешным
            }
            Err(e) => steps::klog(&format!(
                "trampoline breakaway failed ({e:#}) — installing in-job"
            )),
        }
    }

    let relaunch = has("/update") || !has("/S");
    match steps::install(&install_dir, version) {
        Ok(()) => {
            steps::klog("install done");
            if relaunch {
                steps::relaunch_app(&install_dir);
            }
        }
        Err(e) => {
            steps::klog(&format!("install FAILED: {e:#}"));
            // Вернуть юзеру работающее приложение: старые файлы целы.
            steps::relaunch_app(&install_dir);
            std::process::exit(5);
        }
    }
}

/// Каталог payload-распаковки по умолчанию — тот же, что у NSIS-эпохи.
pub(crate) fn default_install_dir() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    base.join("Programs").join("KaminIDE-GPUI")
}
