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
mod ui;

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

    let silent = has("/S");
    let relaunch = has("/update") || !silent;

    // Установка — в фоновом потоке; главный поток либо крутит окно (обычный
    // запуск и /update), либо ждёт (тихий /S). Прогресс шлётся через ui::.
    let dir = install_dir.clone();
    let ver = version.to_string();
    let worker = std::thread::spawn(move || {
        let r = steps::install_with_progress(&dir, &ver, ui::set_progress);
        match &r {
            Ok(()) => {
                steps::klog("install done");
                if relaunch {
                    steps::relaunch_app(&dir);
                }
                ui::set_progress(100);
                ui::DONE.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            Err(e) => {
                steps::klog(&format!("install FAILED: {e:#}"));
                steps::relaunch_app(&dir); // вернуть рабочее приложение
                ui::FAILED.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }
        r.is_ok()
    });

    if silent {
        let ok = worker.join().unwrap_or(false);
        if !ok {
            std::process::exit(5);
        }
    } else {
        ui::run_window(version.to_string()); // возвращается по DONE/FAILED
        if ui::FAILED.load(std::sync::atomic::Ordering::Relaxed) {
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
