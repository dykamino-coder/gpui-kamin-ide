//! KaminIDE patch: сигнал «GPU выполнил поставленную работу» как сообщение окну.
//!
//! `IDXGIDevice2::EnqueueSetEvent` отправляет накопленные команды и взводит
//! событие, когда GPU их выполнит (или устройство будет удалено). Ожидание
//! события ведёт пул потоков ОС; его callback только кладёт сообщение в
//! очередь окна, поэтому UI-поток не блокируется и не опрашивает GPU.

use super::*;
use windows::Win32::{
    Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE, LPARAM, WPARAM},
    System::Threading::{
        CreateEventW, INFINITE, RegisterWaitForSingleObject, UnregisterWaitEx, WT_EXECUTEINWAITTHREAD,
        WT_EXECUTEONLYONCE,
    },
    UI::WindowsAndMessaging::PostMessageW,
};

/// Адресат сигнала. Живёт в куче, пока зарегистрировано ожидание пула.
struct Target {
    hwnd: HWND,
    token: usize,
}

/// Одноразовый сигнал о завершении работы GPU, поставленной до его создания.
/// Drop снимает ожидание (дожидаясь уже начатого callback) и закрывает событие.
pub(super) struct GpuSignal {
    event: HANDLE,
    wait: HANDLE,
    target: *mut Target,
}

unsafe extern "system" fn signalled(context: *mut core::ffi::c_void, _timed_out: bool) {
    // SAFETY: context — Box<Target>, который освобождается только после
    // UnregisterWaitEx(INVALID_HANDLE_VALUE), то есть после этого callback.
    let target = unsafe { &*(context as *const Target) };
    // Окно могло закрыться: тогда сообщение просто не доставляется.
    let _ = unsafe {
        PostMessageW(
            Some(target.hwnd),
            WM_GPUI_FRAME_READBACK,
            WPARAM(target.token),
            LPARAM(0),
        )
    };
}

impl GpuSignal {
    /// Отправить команды устройства и прислать окну `WM_GPUI_FRAME_READBACK`
    /// с `token` в WPARAM, когда GPU их выполнит.
    pub(super) fn new(device: &ID3D11Device, hwnd: HWND, token: usize) -> Result<Self> {
        let dxgi: IDXGIDevice2 = device.cast()?;
        // Ручной сброс: событие видят и пул, и прямое ожидание в тестах.
        let event = unsafe { CreateEventW(None, true, false, windows::core::PCWSTR::null())? };
        let target = Box::into_raw(Box::new(Target { hwnd, token }));
        let mut wait = HANDLE::default();
        let registered = unsafe {
            RegisterWaitForSingleObject(
                &mut wait,
                event,
                Some(signalled),
                Some(target as *const _),
                INFINITE,
                // Callback только кладёт сообщение (не блокирует): выполнять его
                // в потоке ожидания и не будить рабочий поток пула.
                WT_EXECUTEONLYONCE | WT_EXECUTEINWAITTHREAD,
            )
        };
        if let Err(error) = registered {
            // SAFETY: ожидание не зарегистрировано, callback не получит target.
            unsafe {
                drop(Box::from_raw(target));
                let _ = CloseHandle(event);
            }
            return Err(error.into());
        }
        let signal = Self {
            event,
            wait,
            target,
        };
        unsafe { dxgi.EnqueueSetEvent(event)? };
        Ok(signal)
    }

    /// Событие для прямого ожидания (тесты).
    #[cfg(test)]
    pub(super) fn event(&self) -> HANDLE {
        self.event
    }
}

impl Drop for GpuSignal {
    fn drop(&mut self) {
        // SAFETY: INVALID_HANDLE_VALUE ждёт завершения начатого callback, после
        // чего target больше никто не читает; событие закрывается последним.
        unsafe {
            let _ = UnregisterWaitEx(self.wait, Some(INVALID_HANDLE_VALUE));
            drop(Box::from_raw(self.target));
            let _ = CloseHandle(self.event);
        }
    }
}

#[cfg(test)]
mod tests {
    // Без glob: `gpui::test` из родительского glob перекрыл бы атрибут `#[test]`.
    use super::{DirectXDevices, DirectXRendererDevices, GpuSignal, HWND};
    use windows::Win32::{Foundation::WAIT_OBJECT_0, System::Threading::WaitForSingleObject};

    #[test]
    fn signal_fires_after_queued_gpu_work_and_drop_releases_the_wait() {
        let device = DirectXDevices::new().unwrap();
        let devices = DirectXRendererDevices::new(&device, false).unwrap();
        let source = super::super::readback::tests::target(&devices, 7, 3, 41);
        let (staging, w, h) = super::super::readback::copy_to_staging(&devices, &source).unwrap();
        let signal = GpuSignal::new(&devices.device, HWND::default(), 9).unwrap();
        assert_eq!(unsafe { WaitForSingleObject(signal.event(), 10_000) }, WAIT_OBJECT_0);
        drop(signal);
        let pixels =
            super::super::readback::read_staging(&devices.device_context, &staging, w, h).unwrap();
        assert_eq!(pixels, vec![41; 7 * 3 * 4]);
    }

    #[test]
    fn dropping_an_unsignalled_wait_does_not_block() {
        let device = DirectXDevices::new().unwrap();
        let devices = DirectXRendererDevices::new(&device, false).unwrap();
        for token in 0..64 {
            drop(GpuSignal::new(&devices.device, HWND::default(), token).unwrap());
        }
    }
}
