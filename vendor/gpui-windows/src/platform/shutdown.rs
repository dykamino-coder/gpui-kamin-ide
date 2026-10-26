//! Завершение оконных задач и GPU-потоков до остановки цикла сообщений.

use super::*;
use std::os::windows::io::AsRawHandle;
use windows::Win32::System::Threading::{INFINITE, WaitForSingleObject};

/// Считает живые оконные ресурсы, включая отложенное закрытие диалогов Windows.
pub(crate) struct WindowLifetime(Rc<Cell<usize>>);

impl WindowLifetime {
    pub(crate) fn new(count: Rc<Cell<usize>>) -> Self {
        count.set(count.get() + 1);
        Self(count)
    }
}

impl Drop for WindowLifetime {
    fn drop(&mut self) {
        self.0.set(self.0.get() - 1);
    }
}

impl WindowsPlatform {
    pub(super) fn finish_shutdown(&self) -> Result<()> {
        if let Some(stop) = self.vsync_stop.borrow_mut().take() {
            stop.store(true, Ordering::Release);
        }
        // WindowsWindow::drop ждёт закрытия диалогов и ставит DestroyWindow в
        // foreground executor. После WM_QUIT эти задачи ещё требуют сообщений.
        while self.live_windows.get() != 0 || !self.raw_window_handles.read().is_empty() {
            self.wait_for_shutdown_work(None)?;
        }
        for thread in self.vsync_threads.take() {
            let handle = HANDLE(thread.as_raw_handle());
            // Восстановление устройства может послать синхронный SendMessage;
            // обычный join на UI-потоке заблокировал бы отправителя навсегда.
            // Сигнал HANDLE включает TLS-деструкторы, is_finished() — ещё нет.
            while !self.wait_for_shutdown_work(Some(handle))? {}
            if thread.join().is_err() {
                log::error!("VSyncProvider panicked during shutdown");
            }
        }
        self.detach_gpu();
        Ok(())
    }

    fn wait_for_shutdown_work(&self, thread: Option<HANDLE>) -> Result<bool> {
        let handles = thread.map(|handle| [handle]);
        // Поток и platform HWND принадлежат этой платформе и живы до возврата.
        unsafe {
            let result = MsgWaitForMultipleObjectsEx(
                handles.as_ref().map(|handles| handles.as_slice()),
                INFINITE,
                QS_ALLINPUT,
                MWMO_INPUTAVAILABLE,
            );
            if result == WAIT_FAILED {
                return Err(windows::core::Error::from_thread())
                    .context("Waiting for GPUI shutdown");
            }
            let stopped = thread.is_some() && result == WAIT_OBJECT_0;
            let mut msg = MSG::default();
            if !stopped
                && PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool()
                && msg.message != WM_QUIT
            {
                dispatch_message(&msg);
            }
            Ok(stopped)
        }
    }
}

pub(super) fn thread_has_exited(thread: &std::thread::JoinHandle<()>) -> bool {
    // JoinHandle держит HANDLE живым; сигнал включает нативные TLS-деструкторы.
    unsafe { WaitForSingleObject(HANDLE(thread.as_raw_handle()), 0) == WAIT_OBJECT_0 }
}

pub(super) fn dispatch_message(msg: &MSG) {
    unsafe {
        // KaminIDE patch: окна gpui — toplevel; WS_CHILD-окно в нашем
        // потоке — чужое (composition input window WebView2/CEF). Ему
        // нельзя слать WM_GPUI_KEYDOWN: чужой wndproc вернёт 0 из
        // DefWindowProc, `translate_accelerator` сочтёт клавишу
        // обработанной и она не дойдёт ни до TranslateMessage, ни до
        // DispatchMessage. Чужим окнам — обычная пара Translate+Dispatch.
        let foreign_child = (get_window_long(msg.hwnd, GWL_STYLE) as u32 & WS_CHILD.0) != 0;
        if foreign_child || translate_accelerator(msg).is_none() {
            _ = TranslateMessage(msg);
            DispatchMessageW(msg);
        }
    }
}
