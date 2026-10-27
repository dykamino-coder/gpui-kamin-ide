//! Владение Windows-окном, offscreen-размещением и памятью процесса.

use super::{
    args::Args,
    run,
    view::{Phases, View},
};
use gpui::{
    AppContext, Bounds, TitlebarOptions, WindowBackgroundAppearance, WindowBounds,
    WindowDecorations, WindowOptions, point, px, size,
};
use kamin_html::{BROWSER_CSS, Document};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{cell::RefCell, rc::Rc};

pub(super) fn peak_memory() -> Result<u64, Box<dyn std::error::Error>> {
    use windows::Win32::System::{
        ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
        Threading::GetCurrentProcess,
    };
    let mut counters = PROCESS_MEMORY_COUNTERS {
        cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        ..Default::default()
    };
    // Буфер заданного размера принадлежит этому вызову; псевдодескриптор не закрывается.
    unsafe {
        GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb)?;
    }
    Ok(counters.PeakWorkingSetSize as u64)
}

pub(super) fn park(window: &gpui::Window) -> Result<(), Box<dyn std::error::Error>> {
    use windows::Win32::{
        Foundation::{HWND, RECT},
        UI::WindowsAndMessaging::{
            GWL_EXSTYLE, GetSystemMetrics, GetWindowLongPtrW, GetWindowRect, SM_XVIRTUALSCREEN,
            SW_HIDE, SW_SHOWNOACTIVATE, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER,
            SetWindowLongPtrW, SetWindowPos, ShowWindow, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
        },
    };
    let raw = HasWindowHandle::window_handle(window)
        .map_err(|error| format!("window handle unavailable: {error}"))?;
    let RawWindowHandle::Win32(handle) = raw.as_raw() else {
        return Err("expected a Win32 window handle".into());
    };
    let hwnd = HWND(handle.hwnd.get() as *mut _);
    // Живой HWND этого окна; размеры и вид окна сохраняются при переносе.
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
        let mut rect = RECT::default();
        GetWindowRect(hwnd, &mut rect)?;
        let x = GetSystemMetrics(SM_XVIRTUALSCREEN) - (rect.right - rect.left) - 64;
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(
            hwnd,
            GWL_EXSTYLE,
            (ex | WS_EX_TOOLWINDOW.0 as isize) & !(WS_EX_APPWINDOW.0 as isize),
        );
        SetWindowPos(
            hwnd,
            None,
            x,
            60,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        )?;
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    }
    Ok(())
}

pub(super) fn launch(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let preference = gpui_windows::GpuAdapterPreference::parse(&args.gpu)?;
    // Ошибки имени возвращаем до входа в событийный цикл платформы.
    if matches!(preference, gpui_windows::GpuAdapterPreference::ByName(_)) {
        gpui_windows::gpu_adapters(&preference)?;
    }
    // До запуска GPUI потоки приложения ещё не созданы; масштаб фиксирует геометрию снимка.
    unsafe {
        std::env::set_var("GPUI_NO_ERROR_DIALOG", "1");
    }
    let mut instrumentation = kamin_html::host::instrumentation::from_env();
    instrumentation.scale_factor = Some(1.);
    let (capture, sink) = kamin_html::host::capture::CaptureSession::open();
    let outcome = Rc::new(RefCell::new(None));
    let result = outcome.clone();
    let platform = Rc::new(gpui_windows::WindowsPlatform::with_gpu_adapter(
        false, preference,
    )?);
    gpui::Application::with_platform(platform.clone())
        .with_instrumentation(instrumentation)
        .run(move |cx| {
            let setup = || -> Result<_, Box<dyn std::error::Error>> {
                cx.set_text_rendering_mode(gpui::TextRenderingMode::Grayscale);
                let fonts = kamin_html::metrics::FontContext::new(cx.text_system().clone());
                let empty = Document::builder("", BROWSER_CSS).fonts(&fonts).build()?;
                let window = cx.open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds {
                            origin: point(px(40.), px(40.)),
                            size: size(px(args.viewport.0 as f32), px(args.viewport.1 as f32)),
                        })),
                        titlebar: Some(TitlebarOptions {
                            appears_transparent: true,
                            ..Default::default()
                        }),
                        window_decorations: Some(WindowDecorations::Client),
                        window_background: WindowBackgroundAppearance::Opaque,
                        is_resizable: false,
                        is_minimizable: false,
                        focus: false,
                        ..Default::default()
                    },
                    |window, cx| {
                        window
                            .set_frame_capture(Some(sink))
                            .expect("Windows GPU readback");
                        cx.new(|_| View {
                            doc: empty,
                            phases: Rc::new(RefCell::new(Phases::default())),
                        })
                    },
                )?;
                window.update(cx, |_, window, _| park(window))??;
                Ok((window, fonts))
            };
            match setup() {
                Ok((window, fonts)) => {
                    cx.spawn(async move |cx| {
                        let value = run::run(cx, window, &fonts, &args, &platform, &capture)
                            .await
                            .map_err(|e| e.to_string());
                        *result.borrow_mut() = Some(value);
                        let _ = cx.update(|cx| cx.quit());
                    })
                    .detach();
                }
                Err(error) => {
                    *result.borrow_mut() = Some(Err(error.to_string()));
                    cx.quit();
                }
            }
        });
    outcome
        .borrow_mut()
        .take()
        .ok_or("benchmark exited before completion")?
        .map_err(Into::into)
}
