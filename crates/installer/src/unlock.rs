//! Ожидание, пока файлы установленной версии реально освободятся.
//!
//! 2026-10-13 (приёмка 1.0.67): `/update` упал на `chrome_elf.dll` с
//! «Отказано в доступе», хотя лог писал «unlocked on attempt 1». Старая проба
//! переименовывала главный exe — но Windows разрешает переименовать exe
//! работающего процесса, поэтому проба проходила, пока процесс ещё жил и
//! держал загруженные DLL, а перезаписать загруженную DLL нельзя. Теперь:
//! ждём исчезновения процессов и пробуем открыть на запись каждый exe/dll.

use std::path::{Path, PathBuf};
use std::time::Duration;

/// Образы, которые установщик гасит перед распаковкой.
pub const IMAGES: [&str; 3] = ["kaminide-gpui.exe", "kaminhost.exe", "kaminide-web.exe"];

/// Жив ли ещё хоть один процесс с таким образом (по `tasklist`).
fn image_running(image: &str) -> bool {
    let filter = format!("IMAGENAME eq {image}");
    match crate::steps::quiet_cmd("tasklist", &["/FI", &filter, "/NH"]).output() {
        Ok(out) => String::from_utf8_lossy(&out.stdout)
            .to_ascii_lowercase()
            .contains(&image.to_ascii_lowercase()),
        Err(_) => false,
    }
}

/// Ждать (до `secs` секунд), пока погашенные процессы не исчезнут.
pub fn wait_processes_gone(secs: u32) -> bool {
    for _ in 0..secs * 2 {
        if !IMAGES.iter().any(|i| image_running(i)) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    false
}

/// Скачанный инсталлятор и его трамплин-копия лежат в каталоге приложения
/// (так их кладёт встроенный апдейтер, `updater.rs`): payload их не
/// перезаписывает, а запущенный инсталлятор занят САМ собой.
fn is_installer(p: &Path) -> bool {
    let name = p
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    (name.starts_with("kaminide_") && name.ends_with("-setup.exe"))
        || name == "kaminide-setup.exe"
        || name == "kaminide-selfupdate.exe"
}

/// exe/dll верхнего уровня каталога приложения (их держат запущенные процессы),
/// кроме самого инсталлятора и его копий.
///
/// 2026-10-14: проба включала запущенный инсталлятор, который апдейтер
/// скачивает в этот же каталог; он занят собой навсегда — установка из IDE
/// висела 20 с на 8 % и откатывалась на старую версию.
fn binaries(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let me = std::env::current_exe().ok();
    rd.flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("exe") || e.eq_ignore_ascii_case("dll"))
        })
        .filter(|p| !is_installer(p) && me.as_deref() != Some(p.as_path()))
        .collect()
}

/// Первый файл, который нельзя открыть на запись (загруженный образ, чужой
/// хэндл), или `None`, если все свободны.
pub fn first_locked(dir: &Path) -> Option<PathBuf> {
    binaries(dir)
        .into_iter()
        .find(|p| std::fs::OpenOptions::new().write(true).open(p).is_err())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_files_are_not_reported_locked() {
        let dir = std::env::temp_dir().join(format!("kamin-unlock-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.dll"), b"x").unwrap();
        std::fs::write(dir.join("notes.txt"), b"x").unwrap();
        assert_eq!(first_locked(&dir), None);
        assert_eq!(binaries(&dir).len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_running_installer_in_the_app_dir_is_not_probed() {
        let dir = std::env::temp_dir().join(format!("kamin-unlock-self-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for name in [
            "kaminide-gpui.exe",
            "KaminIDE_1.0.69_x64-setup.exe",
            "kaminide-selfupdate.exe",
            "kaminide-setup.exe",
        ] {
            std::fs::write(dir.join(name), b"x").unwrap();
        }
        // Hold the downloaded installer the way a running process would (no write share).
        let held = std::fs::OpenOptions::new()
            .read(true)
            .open(dir.join("KaminIDE_1.0.69_x64-setup.exe"))
            .unwrap();
        let names: Vec<_> = binaries(&dir)
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["kaminide-gpui.exe".to_string()]);
        assert_eq!(first_locked(&dir), None);
        drop(held);
        std::fs::remove_dir_all(&dir).ok();
    }
}
