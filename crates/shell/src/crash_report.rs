//! Отчёт о падении процесса: код, адрес и модуль.
//!
//! Обычная паника Rust печатает место сама, а вот падение в чужом коде (D3D11,
//! CEF) уносит процесс молча — в логе просто обрывается строка. Ставим свой
//! перехватчик системных исключений и печатаем, ЧТО и ГДЕ упало: по имени
//! модуля сразу видно, наш это код, драйвер видеокарты или Chromium.

#[cfg(windows)]
pub fn install() {
    use windows::Win32::System::Diagnostics::Debug::SetUnhandledExceptionFilter;

    unsafe {
        SetUnhandledExceptionFilter(Some(on_exception));
        // База модуля: адреса в отчёте от неё и считаются. Разобрать их в
        // имена можно снаружи:
        //   llvm-symbolizer --obj=<exe> --adjust-vma=<база> <адрес>
        let base = windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
            .map(|h| h.0 as usize)
            .unwrap_or(0);
        println!("[КРАХ] база модуля 0x{base:016X}");
    }
}

#[cfg(not(windows))]
pub fn install() {}

#[cfg(windows)]
unsafe extern "system" fn on_exception(
    info: *const windows::Win32::System::Diagnostics::Debug::EXCEPTION_POINTERS,
) -> i32 {
    use std::io::Write as _;

    let (code, address) = unsafe {
        match info.as_ref().and_then(|i| i.ExceptionRecord.as_ref()) {
            Some(record) => (record.ExceptionCode.0, record.ExceptionAddress as usize),
            None => (0, 0),
        }
    };

    // Модуль по адресу: он и называет виновника.
    let module = unsafe { module_of(address) };

    // Пишем в stderr напрямую: аллокатор и форматтер после порчи памяти могут
    // не пережить обычный println.
    let text = format!("[КРАХ] код 0x{code:08X}, адрес 0x{address:016X}, модуль {module}\n");
    let _ = std::io::stderr().write_all(text.as_bytes());

    // Цепочка вызовов по модулям: адрес падения часто указывает на системную
    // библиотеку, а виновник — тот, кто её позвал.
    for (depth, frame) in unsafe { stack_frames() }.into_iter().enumerate() {
        let name = unsafe { module_of(frame) };
        let line = format!("[КРАХ]  {depth:>2}. 0x{frame:016X}  {name}\n");
        let _ = std::io::stderr().write_all(line.as_bytes());
    }
    let _ = std::io::stderr().flush();
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
        String::from_utf16_lossy(&buf[..len])
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
