//! Capture a restored client area without accepting minimized title-bar images.

/// Снимок клиентской области окна: массив байт BGRA и его размеры.
pub(super) fn capture(hwnd: isize) -> Option<(u32, u32, Vec<u8>)> {
    // `WPT_OFFSCREEN=1`: the frame GPUI itself drew, read back from the GPU
    // (`gpui::frame_capture`) — independent of window position and occlusion.
    if offscreen() {
        let f = gpui::frame_capture::latest()?;
        return Some((f.width, f.height, f.bgra));
    }
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

/// GPU readback capture instead of `PrintWindow` (`WPT_OFFSCREEN=1`).
pub(super) fn offscreen() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("WPT_OFFSCREEN").is_ok_and(|v| v == "1"))
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
