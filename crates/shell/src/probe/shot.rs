//! probe `screenshot` — снимок ТОЛЬКО нашего окна: PrintWindow(PW_RENDERFULLCONTENT)
//! по HWND главного окна процесса. Никакого CopyFromScreen/захвата экрана —
//! чужие окна в кадр попасть не могут по построению.

#![cfg(windows)]

use std::path::Path;

use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleBitmap, CreateCompatibleDC,
    DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SelectObject,
};
use windows::Win32::Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow};

/// Нет в windows 0.62 (есть только PW_CLIENTONLY): недокументированный,
/// но стабильный флаг — без него DirectComposition/DX-контент чёрный.
const PW_RENDERFULLCONTENT: PRINT_WINDOW_FLAGS = PRINT_WINDOW_FLAGS(2);
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClientRect, GetWindowThreadProcessId, IsWindowVisible,
};
use windows::core::BOOL;

/// Первое видимое top-level окно нашего процесса, НЕ TOOLWINDOW
/// (т.е. главное окно, а не overlay-слой). `want_toolwindow` инвертирует:
/// вернёт overlay-окно (WS_EX_TOOLWINDOW).
pub fn find_window(want_toolwindow: bool) -> Option<HWND> {
    use windows::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetWindowLongPtrW, WS_EX_TOOLWINDOW,
    };
    struct FindState {
        pid: u32,
        want_tool: bool,
        found: Option<HWND>,
        // Площадь найденного: у процесса бывают И другие видимые окна
        // (всплывающий тултип gpui — 181×24), и «первое подходящее» давало
        // кадр тултипа вместо окна приложения (ревью ц.12)
        best_area: i64,
    }
    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let state = unsafe { &mut *(lparam.0 as *mut FindState) };
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        if pid == state.pid && unsafe { IsWindowVisible(hwnd) }.as_bool() {
            let ex = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
            let is_tool = ex & WS_EX_TOOLWINDOW.0 != 0;
            if is_tool == state.want_tool {
                use windows::Win32::Foundation::RECT;
                use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;
                let mut r = RECT::default();
                let area = if unsafe { GetWindowRect(hwnd, &mut r) }.is_ok() {
                    i64::from(r.right - r.left) * i64::from(r.bottom - r.top)
                } else {
                    0
                };
                if area > state.best_area {
                    state.best_area = area;
                    state.found = Some(hwnd);
                }
            }
        }
        BOOL(1)
    }
    let mut state = FindState {
        pid: std::process::id(),
        want_tool: want_toolwindow,
        found: None,
        best_area: 0,
    };
    unsafe {
        let _ = EnumWindows(
            Some(enum_proc),
            LPARAM(std::ptr::from_mut(&mut state) as isize),
        );
    }
    state.found
}

/// Список child-окон main (класс + rect + visible) — диагностика wv2.
pub fn list_children() -> Vec<serde_json::Value> {
    use windows::Win32::UI::WindowsAndMessaging::{EnumChildWindows, GetClassNameW, GetWindowRect};
    let Some(main) = find_window(false) else {
        return Vec::new();
    };
    struct S(Vec<serde_json::Value>);
    unsafe extern "system" fn cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let s = unsafe { &mut *(lparam.0 as *mut S) };
        let mut cls = [0u16; 128];
        let n = unsafe { GetClassNameW(hwnd, &mut cls) } as usize;
        let class = String::from_utf16_lossy(&cls[..n]);
        let mut rc = RECT::default();
        let _ = unsafe { GetWindowRect(hwnd, &mut rc) };
        let vis = unsafe { IsWindowVisible(hwnd) }.as_bool();
        s.0.push(serde_json::json!({
            "class": class,
            "rect": [rc.left, rc.top, rc.right - rc.left, rc.bottom - rc.top],
            "visible": vis,
        }));
        BOOL(1)
    }
    let mut s = S(Vec::new());
    unsafe {
        let _ = EnumChildWindows(
            Some(main),
            Some(cb),
            LPARAM(std::ptr::from_mut(&mut s) as isize),
        );
    }
    s.0
}

/// PNG нашего окна → файл. Возвращает (width, height).
/// `overlay=true` снимает overlay-окно (TOOLWINDOW) вместо главного.
pub fn capture_to_png_ex(path: &Path, overlay: bool) -> Result<(u32, u32), String> {
    let hwnd = find_window(overlay).ok_or("window not found")?;
    unsafe {
        let mut rect = RECT::default();
        GetClientRect(hwnd, &mut rect).map_err(|e| e.to_string())?;
        let w = rect.right - rect.left;
        let h = rect.bottom - rect.top;
        if w <= 0 || h <= 0 {
            return Err("empty client rect".into());
        }

        let screen_dc = GetDC(Some(hwnd));
        let mem_dc = CreateCompatibleDC(Some(screen_dc));
        let bitmap = CreateCompatibleBitmap(screen_dc, w, h);
        let old = SelectObject(mem_dc, bitmap.into());

        // PW_RENDERFULLCONTENT — иначе DirectComposition-контент (GPUI/DX) чёрный
        let ok = PrintWindow(hwnd, mem_dc, PW_RENDERFULLCONTENT);

        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h, // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        let lines = GetDIBits(
            mem_dc,
            bitmap,
            0,
            h as u32,
            Some(pixels.as_mut_ptr().cast()),
            &mut info,
            DIB_RGB_COLORS,
        );

        SelectObject(mem_dc, old);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(mem_dc);
        ReleaseDC(Some(hwnd), screen_dc);

        if !ok.as_bool() || lines == 0 {
            return Err("PrintWindow/GetDIBits failed".into());
        }

        // BGRA → RGBA
        for px in pixels.chunks_exact_mut(4) {
            px.swap(0, 2);
            px[3] = 0xff;
        }

        let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
        let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w as u32, h as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().map_err(|e| e.to_string())?;
        writer
            .write_image_data(&pixels)
            .map_err(|e| e.to_string())?;
        Ok((w as u32, h as u32))
    }
}

/// Скрин ЗОНЫ РЕАЛЬНОГО ЭКРАНА (BitBlt с display DC) — диагностика
/// оверлей-геометрии: PrintWindow не показывает композицию DWM.
#[cfg(windows)]
pub fn capture_screen_zone(
    path: &Path,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> Result<(u32, u32), String> {
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleDC, CreateDIBSection,
        DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, ReleaseDC, SRCCOPY, SelectObject,
    };
    unsafe {
        let screen = GetDC(None);
        let mem = CreateCompatibleDC(Some(screen));
        let bi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let bmp = CreateDIBSection(Some(mem), &bi, DIB_RGB_COLORS, &mut bits, None, 0)
            .map_err(|e| e.to_string())?;
        let old = SelectObject(mem, bmp.into());
        BitBlt(mem, 0, 0, w, h, Some(screen), x, y, SRCCOPY).map_err(|e| e.to_string())?;
        let n = (w * h) as usize;
        let mut rgba = vec![0u8; n * 4];
        let src = std::slice::from_raw_parts(bits as *const u8, n * 4);
        for i in 0..n {
            rgba[i * 4] = src[i * 4 + 2];
            rgba[i * 4 + 1] = src[i * 4 + 1];
            rgba[i * 4 + 2] = src[i * 4];
            rgba[i * 4 + 3] = 255;
        }
        SelectObject(mem, old);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(mem);
        ReleaseDC(None, screen);
        let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
        let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w as u32, h as u32);
        enc.set_color(png::ColorType::Rgba);
        let mut wr = enc.write_header().map_err(|e| e.to_string())?;
        wr.write_image_data(&rgba).map_err(|e| e.to_string())?;
        let _ = bi;
        Ok((w as u32, h as u32))
    }
}
