//! Внешние тосты — порт `kamin-ide/src-tauri/src/toast.rs` + `toast-card.ts`.
//!
//! Каждый тост это ОТДЕЛЬНОЕ окно: без рамки, прозрачное, поверх всех,
//! не в таскбаре, без фокуса. Стопка растёт снизу вверх у правого края
//! рабочей области монитора ПОД КУРСОРОМ. Здесь живут геометрия стопки,
//! очередь с бейджем «+N», авто-закрытие 8 с с паузой по ховеру и
//! перекладка после закрытия; сама карточка — `ui/toast_card.rs`.

use crate::ui::toast_card::ToastView;
mod stack;
mod win32;

pub use win32::{cursor_over, focus_main, raw_hwnd};

use stack::{ensure_ticker, max_visible, publish_overflow, pump_queue, relayout, slot_position};
use std::collections::VecDeque;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};
use win32::move_window;

use gpui::prelude::*;
use gpui::{
    AnyWindowHandle, App, Bounds, TitlebarOptions, WindowBackgroundAppearance, WindowBounds,
    WindowKind, WindowOptions, px, size,
};

/// Геометрия стопки (`toast.rs:20-24` оригинала) — логические px.
pub(crate) const WIDTH: f32 = 380.0;
pub(crate) const HEIGHT: f32 = 140.0;
pub(crate) const MARGIN: f32 = 16.0;
pub(crate) const STEP: f32 = 150.0;
pub(crate) const HARD_MAX_VISIBLE: usize = 8;
/// `AUTO_DISMISS` + длительность полосы `animation: shrink 8000ms`.
pub const AUTO_DISMISS: Duration = Duration::from_millis(8000);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Warning,
    Error,
}

impl ToastKind {
    pub fn parse(s: &str) -> ToastKind {
        match s {
            "success" => ToastKind::Success,
            "warning" => ToastKind::Warning,
            "error" => ToastKind::Error,
            _ => ToastKind::Info,
        }
    }

    /// `accentFor` (`toast-card.ts:8-13`).
    pub fn accent(self, p: &kamin_theme::Palette) -> gpui::Rgba {
        crate::colors::rgba(match self {
            ToastKind::Success => p.accent_green,
            ToastKind::Warning => p.accent_yellow,
            ToastKind::Error => p.accent_red,
            ToastKind::Info => p.accent_blue,
        })
    }
}

#[derive(Clone)]
pub struct ToastOpts {
    pub kind: ToastKind,
    pub title: String,
    pub message: String,
    pub sticky: bool,
    pub actions: Vec<String>,
}

struct Active {
    id: u64,
    window: AnyWindowHandle,
    view: gpui::WeakEntity<ToastView>,
}

#[derive(Default)]
pub(crate) struct Inner {
    active: Vec<Active>,
    queue: VecDeque<ToastOpts>,
    next_id: u64,
}

pub(crate) static STATE: LazyLock<Mutex<Inner>> = LazyLock::new(|| Mutex::new(Inner::default()));

/// Без монитора под курсором — границы основного дисплея. Рабочей области
/// (без таскбара) тут нет, и это честная деградация, а не «как в оригинале».
pub(crate) fn fallback_area(cx: &App) -> (f32, f32, f32, f32) {
    match cx.primary_display() {
        Some(d) => {
            let b = d.bounds();
            (
                f32::from(b.origin.x),
                f32::from(b.origin.y),
                f32::from(b.size.width),
                f32::from(b.size.height),
            )
        }
        None => (0.0, 0.0, 1280.0, 720.0),
    }
}

/// Показать тост: свободный слот — окно сразу, иначе в очередь.
pub fn show(opts: ToastOpts, cx: &mut App) {
    let max = max_visible(cx);
    let spawn = {
        let Ok(mut inner) = STATE.lock() else { return };
        if inner.active.len() >= max {
            inner.queue.push_back(opts);
            None
        } else {
            inner.next_id += 1;
            Some((inner.next_id, inner.active.len(), opts))
        }
    };
    if let Some((id, slot, opts)) = spawn {
        open_window(id, slot, opts, cx);
    }
    ensure_ticker(cx);
    publish_overflow(cx);
}

pub(crate) fn open_window(id: u64, slot: usize, opts: ToastOpts, cx: &mut App) {
    let (x, y) = slot_position(cx, slot);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: gpui::point(px(x), px(y)),
            size: size(px(WIDTH), px(HEIGHT)),
        })),
        titlebar: Some(TitlebarOptions {
            title: None,
            appears_transparent: true,
            traffic_light_position: None,
        }),
        // `.focused(false).skip_taskbar(true).always_on_top(true)` оригинала:
        // PopUp у gpui и есть окно поверх всех и вне таскбара
        focus: false,
        show: true,
        kind: WindowKind::PopUp,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    match cx.open_window(options, |window, cx| {
        let view = cx.new(|cx| ToastView::new(id, opts, window, cx));
        if let Ok(mut inner) = STATE.lock() {
            inner.active.push(Active {
                id,
                window: window.window_handle(),
                view: view.downgrade(),
            });
        }
        // Root-обёртка тут не нужна: в карточке нет виджетов gpui-component
        view
    }) {
        // Неактивное PopUp-окно gpui первый кадр сам не запрашивает —
        // без явного `refresh` тост остаётся пустым прямоугольником
        Ok(handle) => {
            // `always_on_top(true)` оригинала: у gpui такой опции нет вовсе,
            // поэтому ставим HWND_TOPMOST сами. Без этого тост уходит ПОД
            // главное окно и «внешним» быть перестаёт
            let _ = handle.update(cx, |_, window, _| {
                move_window(window, x, y);
                window.refresh();
            });
        }
        Err(e) => {
            eprintln!("toast: open_window failed: {e}");
            if let Ok(mut inner) = STATE.lock() {
                inner.active.retain(|a| a.id != id);
            }
        }
    }
}

/// Закрыть тост и подтянуть очередь.
pub fn close(id: u64, cx: &mut App) {
    let handle = {
        let Ok(mut inner) = STATE.lock() else { return };
        match inner.active.iter().position(|a| a.id == id) {
            Some(pos) => inner.active.remove(pos).window,
            None => return,
        }
    };
    let _ = handle.update(cx, |_, window, _| window.remove_window());
    relayout(cx);
    pump_queue(cx);
}

pub(crate) static TICKER: LazyLock<Mutex<bool>> = LazyLock::new(|| Mutex::new(false));

/// Сколько прошло с учётом пауз — доля 1.0 значит «пора закрывать».
pub fn progress(elapsed: Duration) -> f32 {
    (elapsed.as_secs_f32() / AUTO_DISMISS.as_secs_f32()).clamp(0.0, 1.0)
}

/// Момент старта — вынесен, чтобы `ToastView` не тянул `Instant` в API.
pub fn now() -> Instant {
    Instant::now()
}
