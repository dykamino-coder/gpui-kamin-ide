//! Открытие общих текстур Chromium в нашем устройстве D3D11.
//!
//! CEF отдаёт кадр номером общего буфера. Открыть его нужно в ТОМ ЖЕ
//! устройстве, которым рисуется интерфейс, иначе текстуру не показать.

use super::gpu_texture::GpuTexture;
use std::sync::atomic::Ordering;

/// Открыть текстуру буфера. БЕЗ КЭША по номеру — намеренно: номер хэндла ОС
/// переиспользует для НОВОЙ текстуры после закрытия старой, и кэш отдавал
/// мёртвую текстуру с застывшим кадром — на скролле контент «отскакивал на
/// пару кадров назад» (стенд `check_web_scroll`: 2000 → 1818; чем больше был
/// кэш, тем чаще). Открытие через ядро стоит микросекунды — это дешевле
/// одного неверного кадра.
pub(crate) fn open_by_handle(device: isize, handle: isize) -> Option<GpuTexture> {
    let raw = open_shared(device as *mut _, handle)?;
    unsafe { GpuTexture::from_owned(raw) }
}

/// Пожаловаться один раз: в отрисовке нельзя сыпать в лог каждый кадр.
pub(crate) fn warn_once(what: &str) {
    static ONCE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
    if ONCE.swap(false, Ordering::Relaxed) {
        println!("[cef] ускоренный путь недоступен: {what}");
    }
}

/// Открыть общий номер буфера в устройстве gpui и получить свою текстуру.
///
/// `device_raw` — сырой `ID3D11Device` окна; берём его у gpui, чтобы текстура
/// оказалась в ТОМ ЖЕ устройстве, которым рисуется интерфейс.
#[cfg(windows)]
fn open_shared(device_raw: *mut std::ffi::c_void, handle: isize) -> Option<*mut std::ffi::c_void> {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11Device1, ID3D11Texture2D};
    use windows::core::Interface;

    if device_raw.is_null() || handle == 0 {
        return None;
    }
    unsafe {
        // Указатель окна нам одолжен — оборачиваем и возвращаем как было.
        let device: ID3D11Device = Interface::from_raw(device_raw);
        let opened = (|| {
            let device1: ID3D11Device1 = device.cast().ok()?;
            // CEF 150 отдаёт номер нового образца, но на части драйверов
            // приходит старый — пробуем оба, иначе кадр не показать вовсе.
            match device1.OpenSharedResource1::<ID3D11Texture2D>(HANDLE(handle as *mut _)) {
                Ok(texture) => return Some(texture.into_raw()),
                Err(e) => warn_once(&format!("OpenSharedResource1: {e}")),
            }
            let mut legacy: Option<ID3D11Texture2D> = None;
            match device.OpenSharedResource(HANDLE(handle as *mut _), &mut legacy) {
                Ok(()) => legacy.map(|t| t.into_raw()),
                Err(e) => {
                    warn_once(&format!("OpenSharedResource: {e}"));
                    None
                }
            }
        })();
        let _ = Interface::into_raw(device);
        opened
    }
}

#[cfg(not(windows))]
fn open_shared(
    _device_raw: *mut std::ffi::c_void,
    _handle: isize,
) -> Option<*mut std::ffi::c_void> {
    None
}

/// Кэша больше нет — забывать нечего (см. `open_by_handle`).
pub(crate) fn forget_all() {}
