//! Процессы обновляемой копии: выбор по полному пути exe внутри каталога
//! установки, а не по имени образа.
//!
//! INC-2026-0075: `taskkill /F /IM kaminide-gpui.exe` гасил ВСЕ процессы с этим
//! именем — отладочную сборку из соседнего checkout, вторую копию — а ожидание
//! по `tasklist /FI "IMAGENAME eq …"` упиралось в чужой живой процесс до
//! таймаута. Теперь процесс выбирается, только если его образ (пути
//! канонизированы: регистр, `\\?\`, 8.3, junction/symlink) лежит внутри
//! каталога установки. Решение принимается по пути, прочитанному с того же
//! хэндла, которым процесс потом завершается: переиспользованный PID чужого
//! процесса не будет убит по старому снимку.

use std::path::{Path, PathBuf};

/// Итог одной зачистки: что погашено, что не удалось и сколько процессов с
/// тем же именем оставлено как чужие.
#[derive(Debug, Default)]
pub struct KillReport {
    pub killed: Vec<(u32, PathBuf)>,
    pub failed: Vec<(u32, PathBuf)>,
    pub foreign: usize,
}

impl std::fmt::Display for KillReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "killed {}, failed {}, same-name outside install dir left running {}",
            self.killed.len(),
            self.failed.len(),
            self.foreign
        )?;
        for (pid, p) in &self.failed {
            write!(f, "; failed pid {pid} {}", p.display())?;
        }
        Ok(())
    }
}

/// Ключ сравнения пути Windows: без `\\?\`/`\\?\UNC\`, с `\` и без хвостового
/// разделителя, в нижнем регистре (NTFS сравнивает имена без учёта регистра).
pub(crate) fn path_key(p: &Path) -> String {
    let s = p.to_string_lossy().replace('/', "\\");
    let s = if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        s
    };
    s.trim_end_matches('\\').to_lowercase()
}

/// `exe` лежит внутри `dir` на любой глубине (`runtime\kaminhost.exe`).
/// Граница — компонент пути: `C:\App2\x.exe` не внутри `C:\App`.
pub(crate) fn inside(dir: &Path, exe: &Path) -> bool {
    let (d, e) = (path_key(dir), path_key(exe));
    !d.is_empty() && e.len() > d.len() && e.starts_with(&d) && e[d.len()..].starts_with('\\')
}

/// Чистое решение: имя образа из `images` и путь внутри `dir`. Оба пути уже
/// канонизированы через [`resolve`].
pub(crate) fn selected(dir: &Path, images: &[&str], exe: &Path) -> bool {
    let name_matches = exe
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| images.iter().any(|i| i.eq_ignore_ascii_case(n)));
    name_matches && inside(dir, exe)
}

/// Канонический путь (раскрывает 8.3, junction/symlink, регистр); если файла
/// нет — исходный путь, сравнение тогда лексическое.
pub(crate) fn resolve(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// Жив ли хоть один процесс из `images`, чей exe внутри `dir`.
pub fn any_in(dir: &Path, images: &[&str]) -> bool {
    let dir = resolve(dir);
    let mut found = false;
    os::scan(images, false, |_, exe, _| {
        if selected(&dir, images, &resolve(exe)) {
            found = true;
        }
    });
    found
}

/// Завершить процессы из `images`, чей exe внутри `dir`; остальные с тем же
/// именем не трогать.
pub fn kill_in(dir: &Path, images: &[&str]) -> KillReport {
    let dir = resolve(dir);
    let mut report = KillReport::default();
    os::scan(images, true, |pid, exe, terminate| {
        if !selected(&dir, images, &resolve(exe)) {
            report.foreign += 1;
            return;
        }
        if terminate() {
            report.killed.push((pid, exe.to_path_buf()));
        } else {
            report.failed.push((pid, exe.to_path_buf()));
        }
    });
    report
}

#[cfg(windows)]
mod os {
    use std::os::windows::ffi::OsStringExt as _;
    use std::path::PathBuf;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
        QueryFullProcessImageNameW, TerminateProcess,
    };

    struct Owned(HANDLE);
    impl Drop for Owned {
        fn drop(&mut self) {
            // SAFETY: хэндл получен от OpenProcess/CreateToolhelp32Snapshot и
            // закрывается ровно один раз.
            let _ = unsafe { CloseHandle(self.0) };
        }
    }

    fn image_path(h: HANDLE) -> Option<PathBuf> {
        let mut buf = vec![0u16; 32 * 1024];
        let mut len = buf.len() as u32;
        // SAFETY: буфер живёт до конца вызова, `len` — его ёмкость в u16.
        unsafe {
            QueryFullProcessImageNameW(
                h,
                PROCESS_NAME_WIN32,
                windows::core::PWSTR(buf.as_mut_ptr()),
                &mut len,
            )
        }
        .ok()?;
        buf.truncate(len as usize);
        Some(std::ffi::OsString::from_wide(&buf).into())
    }

    /// Обойти процессы с именем из `images` (кроме себя). Для каждого, чей
    /// путь удалось прочитать, `visit(pid, exe, terminate)`; `terminate`
    /// завершает процесс через тот же хэндл, с которого прочитан путь.
    pub fn scan(
        images: &[&str],
        with_terminate: bool,
        mut visit: impl FnMut(u32, &std::path::Path, &mut dyn FnMut() -> bool),
    ) {
        let me = std::process::id();
        // SAFETY: обычный снимок процессов; хэндл закрывает `Owned`.
        let Ok(snap) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
            return;
        };
        let snap = Owned(snap);
        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        // SAFETY: `entry.dwSize` выставлен, структура живёт весь обход.
        let mut more = unsafe { Process32FirstW(snap.0, &mut entry) }.is_ok();
        while more {
            let end = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(260);
            let name = String::from_utf16_lossy(&entry.szExeFile[..end]);
            let pid = entry.th32ProcessID;
            if pid != me && images.iter().any(|i| i.eq_ignore_ascii_case(&name)) {
                let mut access = PROCESS_QUERY_LIMITED_INFORMATION;
                if with_terminate {
                    access |= PROCESS_TERMINATE;
                }
                // Без прав на процесс (другой пользователь/elevated) путь
                // неизвестен — процесс не выбирается: убить его всё равно нельзя.
                // SAFETY: открываем процесс по PID из снимка, хэндл закрывает `Owned`.
                if let Ok(h) = unsafe { OpenProcess(access, false, pid) } {
                    let h = Owned(h);
                    if let Some(exe) = image_path(h.0) {
                        // SAFETY: хэндл открыт с PROCESS_TERMINATE, когда это нужно.
                        let mut terminate =
                            || with_terminate && unsafe { TerminateProcess(h.0, 1) }.is_ok();
                        visit(pid, &exe, &mut terminate);
                    }
                }
            }
            // SAFETY: тот же снимок и структура.
            more = unsafe { Process32NextW(snap.0, &mut entry) }.is_ok();
        }
    }
}

#[cfg(not(windows))]
mod os {
    /// Установщик существует только для Windows; на других ОС процессов нет.
    pub fn scan(
        _images: &[&str],
        _with_terminate: bool,
        _visit: impl FnMut(u32, &std::path::Path, &mut dyn FnMut() -> bool),
    ) {
    }
}

#[cfg(test)]
#[path = "procs_tests.rs"]
mod tests;
