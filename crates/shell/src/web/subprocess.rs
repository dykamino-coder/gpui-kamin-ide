//! Exe дочерних процессов CEF: копия помощника `kaminide-web.exe` под именем
//! главного exe в подпапке `web\`.
//!
//! Диспетчер задач группирует по ИМЕНИ файла, а строку подписывает по
//! FileDescription: так дети и в группе приложения, и различимы («KaminIDE
//! Web»). Перенесено из `process.rs` без изменений поведения.

/// Подготовить exe дочерних процессов: копия помощника под именем главного.
pub(super) fn prepare() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let helper = dir.join("kaminide-web.exe");
    if !helper.exists() {
        eprintln!("[cef] kaminide-web.exe не найден — дети будут копией главного exe");
        return None;
    }
    let sub_dir = dir.join("web");
    let target = sub_dir.join("kaminide-gpui.exe");
    let fresh = match (helper.metadata(), target.metadata()) {
        (Ok(src), Ok(dst)) => src.len() != dst.len() || src.modified().ok() > dst.modified().ok(),
        _ => true,
    };
    if fresh {
        if std::fs::create_dir_all(&sub_dir).is_err() {
            return None;
        }
        // Копия может быть занята живым ребёнком прошлого запуска — тогда
        // оставляем старую: она того же происхождения.
        let _ = std::fs::remove_file(&target);
        if std::fs::copy(&helper, &target).is_err() && !target.exists() {
            return None;
        }
    }
    // libcef.dll и его свита лежат рядом с ГЛАВНЫМ exe; дети из подпапки
    // найдут их через PATH (наследуется).
    let path = std::env::var("PATH").unwrap_or_default();
    let dir_s = dir.to_string_lossy();
    if !path.split(';').any(|p| p == dir_s) {
        unsafe { std::env::set_var("PATH", format!("{dir_s};{path}")) };
    }
    Some(target)
}
