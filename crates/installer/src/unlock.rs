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

/// exe/dll верхнего уровня каталога приложения (их держат запущенные процессы).
fn binaries(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    rd.flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("exe") || e.eq_ignore_ascii_case("dll"))
        })
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
}
