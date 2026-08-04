//! Жалобы слоя проверки D3D11 — в наш лог.
//!
//! В отладочной сборке устройство создаётся со слоем проверки, и он ловит
//! неправильные вызовы. По умолчанию он не только пишет в отладчик, но и
//! останавливает процесс — а отладчика у нас нет, и приложение просто гибло
//! без объяснений. Останов выключаем, а накопленные сообщения печатаем сами.

/// Забрать и напечатать новые сообщения. Зовётся с потока отрисовки.
#[cfg(windows)]
pub(crate) fn drain(device_raw: *mut std::ffi::c_void) {
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_MESSAGE, D3D11_MESSAGE_SEVERITY_CORRUPTION, D3D11_MESSAGE_SEVERITY_ERROR,
        D3D11_MESSAGE_SEVERITY_WARNING, ID3D11Device, ID3D11InfoQueue,
    };
    use windows::core::Interface;

    if device_raw.is_null() {
        return;
    }
    unsafe {
        let device: ID3D11Device = Interface::from_raw(device_raw);
        if let Ok(queue) = device.cast::<ID3D11InfoQueue>() {
            // Останов процесса выключаем один раз: сообщение полезнее смерти.
            for severity in [
                D3D11_MESSAGE_SEVERITY_CORRUPTION,
                D3D11_MESSAGE_SEVERITY_ERROR,
                D3D11_MESSAGE_SEVERITY_WARNING,
            ] {
                let _ = queue.SetBreakOnSeverity(severity, false);
            }
            let count = queue.GetNumStoredMessages();
            for index in 0..count {
                let mut size = 0usize;
                if queue.GetMessage(index, None, &mut size).is_err() || size == 0 {
                    continue;
                }
                let mut buf = vec![0u8; size];
                let message = buf.as_mut_ptr() as *mut D3D11_MESSAGE;
                if queue.GetMessage(index, Some(message), &mut size).is_err() {
                    continue;
                }
                let text = std::slice::from_raw_parts(
                    (*message).pDescription,
                    (*message).DescriptionByteLength.saturating_sub(1),
                );
                // Через stderr: обычный вывод буферизуется, и при падении
                // сообщение — самое нужное — до файла не доезжает.
                eprintln!("[d3d11] {}", String::from_utf8_lossy(text));
            }
            if count > 0 {
                queue.ClearStoredMessages();
            }
        }
        // Устройство принадлежит окну — владение возвращаем.
        let _ = Interface::into_raw(device);
    }
}

#[cfg(not(windows))]
pub(crate) fn drain(_device_raw: *mut std::ffi::c_void) {}
