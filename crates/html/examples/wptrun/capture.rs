//! Снимок завершённого GPU-кадра либо восстановленной клиентской области.

use gpui::AsyncApp;
use kamin_html::host::capture::CaptureSession;

#[derive(Clone)]
pub(super) struct Capture {
    pub(super) session: Option<CaptureSession>,
    pub(super) offscreen: bool,
    pub(super) first: bool,
    pub(super) ready: bool,
}

pub(super) fn setup() -> (
    Capture,
    Option<Box<dyn gpui::frame_capture::FrameCaptureSink>>,
) {
    let offscreen = std::env::var("WPT_OFFSCREEN").as_deref() == Ok("1");
    let first = std::env::var("WPT_FIRST_FRAME").as_deref() == Ok("1");
    let ready = std::env::var("WPT_READY").as_deref() != Ok("0");
    let (session, sink) = if offscreen || first {
        let (session, sink) = CaptureSession::open();
        (Some(session), Some(sink))
    } else {
        (None, None)
    };
    (
        Capture {
            session,
            offscreen,
            first,
            ready,
        },
        sink,
    )
}

/// Fence the previous document and count each completed GPU draw only once.
pub(super) struct CaptureCursor {
    seq: Option<u64>,
    session: Option<CaptureSession>,
}

impl CaptureCursor {
    pub(super) fn new(capture: &Capture) -> Self {
        Self {
            seq: capture
                .offscreen
                .then(|| capture.session.as_ref().unwrap().sequence()),
            session: capture.session.clone(),
        }
    }

    /// Ждать новый завершённый readback, не опрашивая и не копируя старый кадр.
    pub(super) async fn wait(&self, cx: &AsyncApp) -> bool {
        use futures::future::{Either, select};
        let Some(seq) = self.seq else { return false };
        let event = self.session.as_ref().unwrap().after(seq);
        let timeout = cx
            .background_executor()
            .timer(std::time::Duration::from_millis(500));
        matches!(select(event, timeout).await, Either::Left((Ok(_), _)))
    }

    fn advance(&mut self, seq: u64) -> bool {
        let Some(previous) = self.seq else {
            return true;
        };
        if seq <= previous {
            return false;
        }
        self.seq = Some(seq);
        true
    }

    pub(super) fn next(&mut self, hwnd: isize) -> Option<(u32, u32, Vec<u8>)> {
        if self.seq.is_some() {
            let frame = self.session.as_ref()?.latest()?;
            return self.advance(frame.seq).then_some((
                frame.pixels.width,
                frame.pixels.height,
                frame.pixels.bgra,
            ));
        }
        capture(hwnd)
    }
}

/// Снимок клиентской области окна: массив байт BGRA и его размеры.
pub(super) fn capture(hwnd: isize) -> Option<(u32, u32, Vec<u8>)> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleBitmap, CreateCompatibleDC,
        DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SelectObject,
    };
    use windows::Win32::Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClientRect, IsIconic, SW_SHOWNOACTIVATE, ShowWindowAsync,
    };

    let hwnd = HWND(hwnd as *mut _);
    unsafe {
        // A minimized client area can be a stable 181x24 title-bar bitmap.
        // Never compare it as page output. Restore without taking focus and
        // wait for a subsequent capture at the normal viewport size.
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindowAsync(hwnd, SW_SHOWNOACTIVATE);
            return None;
        }
        let mut rect = Default::default();
        GetClientRect(hwnd, &mut rect).ok()?;
        let (w, h) = (
            (rect.right - rect.left) as u32,
            (rect.bottom - rect.top) as u32,
        );
        if w == 0 || h == 0 {
            return None;
        }
        let screen = GetDC(None);
        let dc = CreateCompatibleDC(Some(screen));
        let bitmap = CreateCompatibleBitmap(screen, w as i32, h as i32);
        let old = SelectObject(dc, bitmap.into());
        // Флаг 3 = PW_RENDERFULLCONTENT: без него аппаратно нарисованное окно
        // снимается пустым.
        let ok = PrintWindow(hwnd, dc, PRINT_WINDOW_FLAGS(3)).as_bool();
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w as i32,
                // Отрицательная высота — строки сверху вниз.
                biHeight: -(h as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        let lines = GetDIBits(
            dc,
            bitmap,
            0,
            h,
            Some(pixels.as_mut_ptr() as *mut _),
            &mut info,
            DIB_RGB_COLORS,
        );
        SelectObject(dc, old);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(dc);
        ReleaseDC(None, screen);
        (ok && lines > 0).then_some((w, h, pixels))
    }
}

/// Park the runner window off-screen (`WPT_OFFSCREEN`) and drop its taskbar
/// button: tool-window ex-style, applied while hidden so the taskbar notices,
/// then shown again without activation at an off-screen origin. The window
/// keeps its normal kind and size (a PopUp kind changed the viewport).
pub(super) fn park_offscreen(hwnd: isize) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetWindowLongPtrW, SW_HIDE, SW_SHOWNOACTIVATE, SWP_FRAMECHANGED,
        SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos, ShowWindow,
        WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
    };
    let hwnd = HWND(hwnd as *mut _);
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let ex = (ex | WS_EX_TOOLWINDOW.0 as isize) & !(WS_EX_APPWINDOW.0 as isize);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex);
        let _ = SetWindowPos(
            hwnd,
            None,
            -4000,
            60,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    }
}

#[cfg(test)]
mod tests {
    use super::CaptureCursor;

    #[test]
    fn only_new_draws_cross_the_document_fence() {
        let mut cursor = CaptureCursor {
            seq: Some(41),
            session: None,
        };
        assert!(!cursor.advance(41));
        assert!(!cursor.advance(40));
        assert!(cursor.advance(42));
        for _ in 0..120 {
            assert!(!cursor.advance(42));
        }
        assert!(cursor.advance(43));
    }
}
