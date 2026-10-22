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

/// Образы, которые установщик гасит перед распаковкой — только если их exe
/// лежит в обновляемом каталоге (`procs`, INC-2026-0075).
pub const IMAGES: [&str; 3] = ["kaminide-gpui.exe", "kaminhost.exe", "kaminide-web.exe"];

/// Ждать (до `secs` секунд), пока процессы установленной копии в `dir` не
/// исчезнут. Одноимённые процессы из других каталогов ожидание не продлевают.
pub fn wait_processes_gone(dir: &Path, secs: u32) -> bool {
    for _ in 0..secs * 2 {
        if !crate::procs::any_in(dir, &IMAGES) {
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

/// Where a busy file is moved before the new one is written:
/// `chrome_elf.dll` → `chrome_elf.dll.kamin-old-<pid>-<n>`.
pub fn aside_path(target: &Path, n: u32) -> PathBuf {
    let mut name = target.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".kamin-old-{}-{n}", std::process::id()));
    target.with_file_name(name)
}

/// Delete files moved aside by earlier runs (anywhere under `dir`). A file that
/// a still-dying process maps stays until the next run.
pub fn remove_aside(dir: &Path) -> usize {
    let mut removed = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(".kamin-old-"))
                && std::fs::remove_file(&p).is_ok()
            {
                removed += 1;
            }
        }
    }
    removed
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

    #[test]
    fn moved_aside_files_are_cleaned_up_and_not_probed() {
        let dir = std::env::temp_dir().join(format!("kamin-unlock-aside-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("runtime")).unwrap();
        let dll = dir.join("chrome_elf.dll");
        std::fs::write(&dll, b"new").unwrap();
        let old = aside_path(&dll, 0);
        assert!(old.to_string_lossy().contains("chrome_elf.dll.kamin-old-"));
        std::fs::write(&old, b"old").unwrap();
        std::fs::write(
            aside_path(&dir.join("runtime").join("kaminhost.exe"), 1),
            b"old",
        )
        .unwrap();
        assert_eq!(binaries(&dir).len(), 1);
        assert_eq!(remove_aside(&dir), 2);
        assert!(dll.is_file() && !old.exists());
        std::fs::remove_dir_all(&dir).ok();
    }
}
