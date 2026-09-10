//! Отчёт о падении процесса: код, адрес, модуль и цепочка вызовов — В ФАЙЛ.
//!
//! Обычная паника Rust печатает место сама, а падение в чужом коде (D3D11,
//! libcef) уносит процесс молча. Раньше перехватчик стоял ТОЛЬКО в главном
//! процессе и писал в stderr — у packaged GUI-сборки консоли нет, поэтому на
//! машине человека не оставалось ничего, кроме системной модалки «KaminIDE has
//! stopped working». Теперь пишем в `<cache>/crash.log`, и ставим перехватчик
//! ещё и в дочерних процессах CEF — падают как раз они.
//!
//! Внутри обработчика работаем по минимуму: адреса, имена модулей и запись
//! в уже открытый файл. Ни аллокаций через свой аллокатор, ни блокировок —
//! память в этот момент уже могла быть испорчена.

#[cfg(any(windows, test))]
mod log_files;
#[cfg(any(windows, test))]
mod renderer_fields;

#[cfg(windows)]
use renderer_fields::{safe_status, view_ref};
#[cfg(windows)]
use std::io::Write as _;

/// Что делать после того, как отчёт записан.
#[derive(Clone, Copy)]
pub enum AfterReport {
    /// Отдать управление системе: она покажет свою модалку. Для главного
    /// процесса — человек должен увидеть, что приложение умерло.
    SystemDialog,
    /// Тихо завершить процесс. Для детей CEF: смерть рендерера — штатная
    /// ситуация, её лечит перезагрузка вью, а модалка только пугает.
    QuitSilently,
}

#[cfg(windows)]
static MODE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
/// Имя процесса в строках лога: `main` или `web` — иначе не отличить, кто упал.
#[cfg(windows)]
static ROLE: std::sync::Mutex<Option<&'static str>> = std::sync::Mutex::new(None);

/// Поставить перехватчик. `role` попадает в каждую строку отчёта.
#[cfg(windows)]
pub fn install(role: &'static str, after: AfterReport) {
    use std::sync::atomic::Ordering;
    use windows::Win32::System::Diagnostics::Debug::{
        SEM_FAILCRITICALERRORS, SEM_NOGPFAULTERRORBOX, SetErrorMode, SetUnhandledExceptionFilter,
    };

    if role == "main" {
        // Один новый current-файл на запуск + три прошлых incident trail.
        // CEF-дети общий файл не вращают: они стартуют параллельно.
        log_files::rotate(&crash_log_path(), 3);
    }
    if let Ok(mut r) = ROLE.lock() {
        *r = Some(role);
    }
    MODE.store(
        match after {
            AfterReport::SystemDialog => 0,
            AfterReport::QuitSilently => 1,
        },
        Ordering::Relaxed,
    );
    unsafe {
        if matches!(after, AfterReport::QuitSilently) {
            // Гасим системный диалог ЗАРАНЕЕ: до нашего фильтра дело доходит
            // не при всякой смерти (быстрый выход, сбой в чужом потоке).
            SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX);
        }
        SetUnhandledExceptionFilter(Some(on_exception));
        let base = windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
            .map(|h| h.0 as usize)
            .unwrap_or(0);
        // База модуля: адреса в отчёте считаются от неё. Разобрать снаружи:
        //   llvm-symbolizer --obj=<exe> --adjust-vma=<база> <адрес>
        write_line(&format!("[КРАХ] {role}: старт, база модуля 0x{base:016X}"));
    }
}

#[cfg(not(windows))]
pub fn install(_role: &'static str, _after: AfterReport) {}

/// Файл отчёта: рядом с `diag.log`, чтобы человек присылал ОДНУ папку.
#[cfg(windows)]
fn crash_log_path() -> std::path::PathBuf {
    let base = std::env::var("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    let dir = base.join("kaminide-gpui-dev").join("cache");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("crash.log")
}

/// Строка в файл отчёта И в stderr (в dev-сборке с консолью так виднее).
#[cfg(windows)]
fn write_line(line: &str) {
    let pid = std::process::id();
    let ts_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let text = format!(
        "[ts_ms {ts_ms}] [version {}] [pid {pid}] {line}\n",
        env!("CARGO_PKG_VERSION")
    );
    let _ = std::io::stderr().write_all(text.as_bytes());
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(crash_log_path())
    {
        let _ = f.write_all(text.as_bytes());
        let _ = f.flush();
    }
}

#[cfg(windows)]
unsafe extern "system" fn on_exception(
    info: *const windows::Win32::System::Diagnostics::Debug::EXCEPTION_POINTERS,
) -> i32 {
    use std::sync::atomic::Ordering;

    let (code, address) = unsafe {
        match info.as_ref().and_then(|i| i.ExceptionRecord.as_ref()) {
            Some(record) => (record.ExceptionCode.0, record.ExceptionAddress as usize),
            None => (0, 0),
        }
    };
    let role = ROLE.lock().ok().and_then(|r| *r).unwrap_or("?");
    // Модуль по адресу: он и называет виновника.
    let module = unsafe { module_of(address) };
    write_line(&format!(
        "[КРАХ] {role}: код 0x{code:08X}, адрес 0x{address:016X}, модуль {module}"
    ));
    // Цепочка вызовов: адрес падения часто указывает на системную библиотеку,
    // а виновник — тот, кто её позвал.
    for (depth, frame) in unsafe { stack_frames() }.into_iter().enumerate() {
        let name = unsafe { module_of(frame) };
        write_line(&format!("[КРАХ]  {depth:>2}. 0x{frame:016X}  {name}"));
    }
    if MODE.load(Ordering::Relaxed) == 1 {
        // 1 = EXCEPTION_EXECUTE_HANDLER: раскрутка прекращается, процесс
        // уходит тихо — модалки не будет.
        return 1;
    }
    // 0 = EXCEPTION_CONTINUE_SEARCH: дальше пусть система делает своё.
    0
}

/// Имя модуля, которому принадлежит адрес.
#[cfg(windows)]
unsafe fn module_of(address: usize) -> String {
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::System::LibraryLoader::{
        GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
        GetModuleFileNameW, GetModuleHandleExW,
    };

    unsafe {
        let mut handle = HMODULE::default();
        let ok = GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            windows::core::PCWSTR(address as *const u16),
            &mut handle,
        );
        if ok.is_err() {
            return String::from("(неизвестный модуль)");
        }
        let mut buf = [0u16; 260];
        let len = GetModuleFileNameW(Some(handle), &mut buf) as usize;
        if len == 0 {
            return String::from("(неизвестный модуль)");
        }
        let path = String::from_utf16_lossy(&buf[..len]);
        std::path::Path::new(path.as_str())
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("(неизвестный модуль)")
            .to_owned()
    }
}

/// Адреса возврата текущей цепочки вызовов.
#[cfg(windows)]
unsafe fn stack_frames() -> Vec<usize> {
    use windows::Win32::System::Diagnostics::Debug::RtlCaptureStackBackTrace;

    let mut frames: [*mut std::ffi::c_void; 24] = [std::ptr::null_mut(); 24];
    let got = unsafe { RtlCaptureStackBackTrace(0, &mut frames, None) } as usize;
    frames[..got].iter().map(|f| *f as usize).collect()
}

/// Записать в тот же файл строку не о падении, а о СМЕРТИ ребёнка: хост видит
/// её через CEF-колбэк, когда сам ребёнок уже ничего записать не может
/// (убит системой, кончилась память).
#[cfg(windows)]
pub fn note(line: &str) {
    write_line(line);
}

#[cfg(not(windows))]
pub fn note(_line: &str) {}

/// Структурированная смерть CEF renderer без свободного текста Chromium.
/// `error_string` намеренно не принимаем: URL/путь/страница могут содержать
/// пользовательские данные. View-id превращается в класс + стабильный hash.
#[cfg(windows)]
pub fn note_renderer_termination(view_id: &str, status: &str, error_code: i32) {
    write_line(&format!(
        "[renderer-terminated] view={} status={} error_code={} samples=incident.log",
        view_ref(view_id),
        safe_status(status),
        error_code
    ));
}

#[cfg(not(windows))]
pub fn note_renderer_termination(_view_id: &str, _status: &str, _error_code: i32) {}
