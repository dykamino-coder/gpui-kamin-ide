//! Интеграция с Windows-шеллом: «Open with KaminIDE» в контекстном меню папок
//! + single-instance форвард открытия папки.
//!
//! Порт `kamin-ide/src-tauri/src/context_menu.rs` (та же семантика self-heal:
//! регистрация на КАЖДОМ запуске указывает на текущий exe — последний
//! запущенный вариант приложения выигрывает меню; так Tauri-версия и
//! GPUI-порт честно перехватывают меню друг у друга).
//!
//! Single-instance: вместо мьютекса — probe-порт 9333. Живой инстанс отвечает
//! на TCP; второй запуск шлёт `{"cmd":"openFolder","path":…}` и выходит.

use std::path::{Path, PathBuf};

#[cfg(windows)]
const MENU_KEYS: [&str; 2] = [
    r"Software\Classes\Directory\shell\OpenWithKaminIDE",
    r"Software\Classes\Directory\Background\shell\OpenWithKaminIDE",
];

/// Одно строковое значение реестра через `reg query`; None — ключа нет.
#[cfg(windows)]
fn read_reg_value(hk_key: &str, name: &str) -> Option<String> {
    use std::os::windows::process::CommandExt as _;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = std::process::Command::new("reg")
        .args(["query", hk_key, "/v", name])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        if let Some(pos) = line.find("REG_SZ") {
            let val = line[pos + "REG_SZ".len()..].trim();
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
    }
    None
}

/// Explorer агрессивно кэширует иконку пункта меню — после смены exe нужен
/// broadcast «ассоциации изменились», иначе висит старая.
#[cfg(windows)]
fn flush_icon_cache() {
    use windows::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify};
    // Safety: null-указатели документированы как «изменилось всё».
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None) };
}

/// Регистрация (self-heal) пункта меню. Best-effort, каждый reg-вызов
/// изолирован. Зовётся из фонового потока — reg.exe небыстрый.
#[cfg(windows)]
pub fn register_context_menu() {
    use std::os::windows::process::CommandExt as _;
    use std::process::Stdio;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let Ok(exe) = std::env::current_exe() else { return };
    let exe_str = exe.display().to_string();
    // `%V` — папка из клика; шелл сам квотит.
    let command = format!("\"{exe_str}\" \"%V\"");

    // Отпечаток exe (путь+mtime): пересборка меняет встроенную иконку при том
    // же пути — только отпечаток покажет, что пора сбрасывать кэш иконок.
    let mtime = std::fs::metadata(&exe)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let fingerprint = format!("{exe_str}|{mtime}");
    let root0 = format!("HKCU\\{}", MENU_KEYS[0]);
    let changed = read_reg_value(&root0, "_Fingerprint").as_deref() != Some(fingerprint.as_str());
    if !changed {
        return; // уже указывает на этот exe этой сборки
    }

    let reg = |args: &[&str]| {
        let _ = std::process::Command::new("reg")
            .args(args)
            .creation_flags(CREATE_NO_WINDOW)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    };
    for key in MENU_KEYS {
        let hk = format!("HKCU\\{key}");
        reg(&["add", &hk, "/ve", "/d", "Open with KaminIDE", "/f"]);
        reg(&["add", &hk, "/v", "Icon", "/d", &exe_str, "/f"]);
        reg(&["add", &format!("{hk}\\command"), "/ve", "/d", &command, "/f"]);
    }
    reg(&["add", &root0, "/v", "_Fingerprint", "/d", &fingerprint, "/f"]);
    flush_icon_cache();
    eprintln!("[win] контекстное меню папок → {command}");
}

#[cfg(not(windows))]
pub fn register_context_menu() {}

/// Папка из argv: первый позиционный аргумент-каталог (флаги и argv[0]
/// пропускаются). Используется и на холодном старте, и для argv второго
/// экземпляра.
pub fn folder_in<I: IntoIterator<Item = String>>(args: I) -> Option<PathBuf> {
    for arg in args {
        if arg.starts_with('-') {
            continue;
        }
        let p = PathBuf::from(&arg);
        if p.is_dir() {
            return Some(p);
        }
    }
    None
}

/// Папка, переданная ЭТОМУ процессу (Explorer «Open with KaminIDE»).
pub fn launch_folder() -> Option<PathBuf> {
    folder_in(std::env::args().skip(1))
}

static LAUNCH_FOLDER: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();

/// Зафиксировать папку холодного старта — прочитает host::connect при спавне
/// сайдкара (`--open-folder`).
pub fn set_launch_folder(folder: Option<PathBuf>) {
    let _ = LAUNCH_FOLDER.set(folder);
}

pub fn stored_launch_folder() -> Option<PathBuf> {
    LAUNCH_FOLDER.get().cloned().flatten()
}

/// Сессия — удалённая (RDP)? Вечные декоративные анимации (пульс точек,
/// мигание курсора, спиннеры чипов) под RDP превращаются в непрерывный поток
/// кадров по сети — «отклик большой неприятный» (жалоба юзера). Метрика
/// дешёвая и живая (переподключение по RDP меняет ответ) — зовём без кэша.
/// `KAMIN_REDUCE_MOTION=1|0` — форс/запрет режима (тестовые стенды без RDP
/// и ручной откат юзером); без переменной — детект RDP.
fn reduce_motion_env() -> Option<bool> {
    match std::env::var("KAMIN_REDUCE_MOTION").ok().as_deref() {
        Some("1") => Some(true),
        Some("0") => Some(false),
        _ => None,
    }
}

#[cfg(windows)]
pub fn reduce_motion() -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_REMOTESESSION};
    reduce_motion_env()
        // Safety: чистый запрос метрики.
        .unwrap_or_else(|| unsafe { GetSystemMetrics(SM_REMOTESESSION) != 0 })
}

#[cfg(not(windows))]
pub fn reduce_motion() -> bool {
    reduce_motion_env().unwrap_or(false)
}

/// Single-instance: если probe-порт живого инстанса отвечает — форвардим ему
/// открытие папки (или просто фокус) и возвращаем true (нам пора выйти).
/// Мёртвый/чужой порт (нет ответа на ping) — false, стартуем сами.
pub fn forward_to_running_instance(folder: Option<&Path>) -> bool {
    use std::io::{BufRead as _, BufReader, Write as _};
    let port: u16 = std::env::var("KAMIN_PROBE_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(9333);
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let timeout = std::time::Duration::from_millis(700);
    let Ok(mut stream) = std::net::TcpStream::connect_timeout(&addr, timeout) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(timeout));
    let req = match folder {
        Some(f) => serde_json::json!({"cmd": "openFolder", "path": f.display().to_string()}),
        None => serde_json::json!({"cmd": "focusWindow"}),
    };
    if writeln!(stream, "{req}").is_err() {
        return false;
    }
    let mut line = String::new();
    if BufReader::new(stream).read_line(&mut line).is_err() || line.trim().is_empty() {
        return false; // порт занят не нами / инстанс умирает — стартуем сами
    }
    eprintln!("[win] живой инстанс принял {}", req["cmd"]);
    true
}
