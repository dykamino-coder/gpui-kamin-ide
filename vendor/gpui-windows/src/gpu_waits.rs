//! KaminIDE patch: ожидание сигналов GPU прямо в цикле сообщений UI-потока.
//!
//! Событие `EnqueueSetEvent` (см. `directx_renderer::gpu_signal`) ждёт сам
//! UI-поток вместе с очередью сообщений (`MsgWaitForMultipleObjectsEx`), как
//! блокирующий `Map` ждал его раньше: ядро будит один поток напрямую, без
//! промежуточного потока пула и `PostMessage`. На двух загруженных ядрах лишний
//! переход стоил кванта планировщика на кадр. Ожидания не опрашиваются: цикл
//! спит, пока нет ни сообщения, ни сигнала.

use std::cell::RefCell;
use windows::Win32::{
    Foundation::{HANDLE, HWND, LPARAM, WAIT_FAILED, WAIT_OBJECT_0, WPARAM},
    System::Threading::INFINITE,
    UI::WindowsAndMessaging::{
        MSG, MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, PM_REMOVE, PeekMessageW,
        QS_ALLINPUT, SendMessageW, WM_QUIT,
    },
};

use crate::WM_GPUI_FRAME_READBACK;

/// Предел `MsgWaitForMultipleObjectsEx`: `MAXIMUM_WAIT_OBJECTS - 1`.
const MAX_WAITS: usize = 63;

/// Зарегистрированный сигнал: окно получит `WM_GPUI_FRAME_READBACK(token)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GpuWait {
    pub(crate) event: HANDLE,
    pub(crate) hwnd: HWND,
    pub(crate) token: usize,
}

thread_local! {
    // Рендереры живут на UI-потоке; регистрирует и ждёт один и тот же поток.
    static WAITS: RefCell<Vec<GpuWait>> = const { RefCell::new(Vec::new()) };
}

/// Ждать `event` в цикле сообщений этого потока.
pub(crate) fn register(wait: GpuWait) {
    WAITS.with_borrow_mut(|waits| waits.push(wait));
}

/// Снять ожидание (событие ещё не закрыто вызывающим).
pub(crate) fn unregister(event: HANDLE) {
    WAITS.with_borrow_mut(|waits| waits.retain(|wait| wait.event != event));
}

/// Что разбудило цикл сообщений.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Wake {
    /// Сработал сигнал GPU; он уже снят с ожидания.
    Gpu(GpuWait),
    /// В очереди есть сообщение (или ожидание не удалось — тогда очередь
    /// обрабатывается как раньше, блокирующим чтением).
    Message,
}

/// Уснуть до сообщения или сигнала GPU; сигналы имеют приоритет (младшие
/// индексы `MsgWaitForMultipleObjectsEx`).
pub(crate) fn wait(timeout_ms: u32) -> Wake {
    let waits: Vec<GpuWait> =
        WAITS.with_borrow(|waits| waits.iter().take(MAX_WAITS).copied().collect());
    let handles: Vec<HANDLE> = waits.iter().map(|wait| wait.event).collect();
    // SAFETY: события закрываются только в Drop сигнала на этом же потоке,
    // который сейчас спит здесь, поэтому все handles живы до возврата.
    let result = unsafe {
        MsgWaitForMultipleObjectsEx(
            (!handles.is_empty()).then_some(handles.as_slice()),
            timeout_ms,
            QS_ALLINPUT,
            MWMO_INPUTAVAILABLE,
        )
    };
    if result == WAIT_FAILED {
        log::error!(
            "waiting for messages and GPU signals failed: {}",
            windows::core::Error::from_thread()
        );
        return Wake::Message;
    }
    let index = result.0.wrapping_sub(WAIT_OBJECT_0.0) as usize;
    match waits.get(index) {
        Some(&wait) => {
            unregister(wait.event);
            Wake::Gpu(wait)
        }
        None => Wake::Message,
    }
}

/// Главный цикл сообщений UI-потока (замена `while GetMessageW`), который
/// заодно доставляет сигналы GPU окнам. Возвращает после `WM_QUIT`.
pub(crate) fn run_message_loop(mut dispatch: impl FnMut(&MSG)) {
    let mut msg = MSG::default();
    loop {
        match wait(INFINITE) {
            // Тот же поток: SendMessageW вызывает оконную процедуру напрямую.
            Wake::Gpu(wait) => unsafe {
                SendMessageW(
                    wait.hwnd,
                    WM_GPUI_FRAME_READBACK,
                    Some(WPARAM(wait.token)),
                    Some(LPARAM(0)),
                );
            },
            Wake::Message => {
                // По одному сообщению: следующий сигнал GPU не ждёт всю очередь.
                if unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE) }.as_bool() {
                    if msg.message == WM_QUIT {
                        return;
                    }
                    dispatch(&msg);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::{
        Foundation::CloseHandle,
        System::Threading::{CreateEventW, SetEvent},
    };

    fn event() -> HANDLE {
        unsafe { CreateEventW(None, true, false, windows::core::PCWSTR::null()) }.unwrap()
    }

    #[test]
    fn signalled_event_wakes_once_and_is_unregistered() {
        let (quiet, ready) = (event(), event());
        let hwnd = HWND::default();
        register(GpuWait {
            event: quiet,
            hwnd,
            token: 1,
        });
        register(GpuWait {
            event: ready,
            hwnd,
            token: 2,
        });
        unsafe { SetEvent(ready) }.unwrap();
        assert_eq!(
            wait(0),
            Wake::Gpu(GpuWait {
                event: ready,
                hwnd,
                token: 2
            })
        );
        // Снятый сигнал больше не будит, хотя событие осталось взведённым.
        assert_eq!(wait(0), Wake::Message);
        unregister(quiet);
        assert!(WAITS.with_borrow(Vec::is_empty));
        unsafe {
            CloseHandle(quiet).unwrap();
            CloseHandle(ready).unwrap();
        }
    }

    #[test]
    fn unregistered_event_never_wakes_the_loop() {
        let ready = event();
        register(GpuWait {
            event: ready,
            hwnd: HWND::default(),
            token: 3,
        });
        unregister(ready);
        unsafe { SetEvent(ready) }.unwrap();
        assert_eq!(wait(0), Wake::Message);
        unsafe { CloseHandle(ready) }.unwrap();
    }
}
