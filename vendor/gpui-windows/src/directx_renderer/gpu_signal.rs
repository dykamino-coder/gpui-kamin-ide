//! KaminIDE patch: сигнал «GPU выполнил поставленную работу» как сообщение окну.
//!
//! `IDXGIDevice2::EnqueueSetEvent` отправляет накопленные команды и взводит
//! событие, когда GPU их выполнит (или устройство будет удалено). Событие ждёт
//! цикл сообщений UI-потока (`gpu_waits`) вместе с очередью окна и по сигналу
//! передаёт окну `WM_GPUI_FRAME_READBACK`; UI-поток не блокируется на GPU и не
//! опрашивает его, а между сигналом и обработкой нет промежуточного потока.

use super::*;
use crate::gpu_waits::{self, GpuWait};
use windows::Win32::{
    Foundation::{CloseHandle, HANDLE},
    System::Threading::CreateEventW,
};

/// Одноразовый сигнал о завершении работы GPU, поставленной до его создания.
/// Drop снимает ожидание с цикла сообщений и закрывает событие.
pub(super) struct GpuSignal {
    event: HANDLE,
}

impl GpuSignal {
    /// Отправить команды устройства и прислать окну `WM_GPUI_FRAME_READBACK`
    /// с `token` в WPARAM, когда GPU их выполнит.
    pub(super) fn new(device: &ID3D11Device, hwnd: HWND, token: usize) -> Result<Self> {
        let dxgi: IDXGIDevice2 = device.cast()?;
        // Ручной сброс: событие остаётся взведённым и для прямого ожидания в тестах.
        let event = unsafe { CreateEventW(None, true, false, windows::core::PCWSTR::null())? };
        let signal = Self { event };
        unsafe { dxgi.EnqueueSetEvent(event)? };
        gpu_waits::register(GpuWait { event, hwnd, token });
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
        // Ожидание снимается до закрытия: цикл сообщений того же потока больше
        // не получит этот HANDLE.
        gpu_waits::unregister(self.event);
        let _ = unsafe { CloseHandle(self.event) };
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
