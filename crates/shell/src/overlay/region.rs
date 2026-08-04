//! Hit-регионы и оконный регион: что в overlay ловит ввод, а что прозрачно.
//!
//! Вынесено без изменения поведения (`plan/100-refactor-250.md`).

use super::subclass::OVERLAY_HWND;
use gpui::prelude::*;
use std::sync::atomic::Ordering;

/// Hit-прямоугольники контента overlay (client-координаты, физические px).
/// Всё ВНЕ них WM_NCHITTEST отдаёт HTTRANSPARENT → ввод (hover/клики)
/// проваливается в main: тултип от другого элемента работает при открытом
/// поповере. Пишутся canvas-хуком на каждый prepaint (генерация очищает).
pub(super) static HIT_RECTS: std::sync::LazyLock<std::sync::Mutex<Vec<[f32; 4]>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(Vec::new()));

/// Невидимый маркер: регистрирует bounds родителя как hit-регион overlay.
/// Вставлять в корень каждого интерактивного оверлея (меню/модалка/тост).
/// Офсет client-области overlay внутри его window-ректа (рамки Zed::Window
/// неснимаемы) — регион (координаты ОКНА) сдвигается на него.
pub(super) static CLIENT_DX: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
pub(super) static CLIENT_DY: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

/// Видимая часть окна (регион): пустота overlay рендерится ЧЁРНЫМ (gpui без
/// альфы на Windows) — окно обрезается до этих ректов.
pub(super) static REGION_RECTS: std::sync::LazyLock<std::sync::Mutex<Vec<[f32; 4]>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(Vec::new()));

fn push_rect(list: &std::sync::Mutex<Vec<[f32; 4]>>, bounds: &gpui::Bounds<gpui::Pixels>, sf: f32) {
    list.lock().unwrap().push([
        f32::from(bounds.origin.x) * sf,
        f32::from(bounds.origin.y) * sf,
        f32::from(bounds.size.width) * sf,
        f32::from(bounds.size.height) * sf,
    ]);
}

/// Снимок hit-регионов overlay для probe: точки ВНЕ них проваливаются в
/// main. Нужно, чтобы ловить «залипшие» регионы (клик по кнопке титлбара
/// съедался невидимым прямоугольником прошлого кадра).
pub fn hit_rects_snapshot() -> Vec<[f32; 4]> {
    HIT_RECTS.lock().unwrap().clone()
}

/// Обычный поповер: и ввод, и видимый регион.
pub fn hit_area() -> impl IntoElement {
    gpui::canvas(
        |bounds, window, _cx| {
            let sf = window.scale_factor();
            push_rect(&HIT_RECTS, &bounds, sf);
            push_rect(&REGION_RECTS, &bounds, sf);
        },
        // paint-фаза идёт после ВСЕХ prepaint кадра — списки полны,
        // можно резать окно (идемпотентно при нескольких хуках)
        |_, _, window, _| {
            apply_window_region(window.scale_factor());
        },
    )
    .absolute()
    .size_full()
}

/// ТОЛЬКО ввод (модальный скрим): окно не расширяется — скрим невидим
/// (60%-затемнение невозможно без альфы), но клики не проваливаются в main.
pub fn input_area() -> impl IntoElement {
    gpui::canvas(
        |bounds, window, _cx| {
            push_rect(&HIT_RECTS, &bounds, window.scale_factor());
        },
        |_, _, window, _| {
            apply_window_region(window.scale_factor());
        },
    )
    .absolute()
    .size_full()
}

/// Видимый регион ТУЛТИПА: пушит rect только когда рендер идёт в
/// overlay-окне (тултипы main-окна не должны попадать в регион overlay).
/// Без этого тултип overlay-контента обрезается SetWindowRgn.
/// Тень дропдаунов (--shadow-dropdown): 0 8 24 rgba(0,0,0,.45).
/// Возможна только в alpha-режиме (dcomp) — в регион-режиме рисуется чёрным.
pub fn dropdown_shadow() -> Vec<gpui::BoxShadow> {
    // Слой живёт в главном окне (dcomp-альфа всегда есть) — тень рисуем всегда.
    // --shadow-dropdown 1:1 (тем-зависимый словарь; ревью ц.1: было 0/8/24/.45)
    crate::ui::shadows::dropdown()
}

pub fn tooltip_region() -> impl IntoElement {
    // Ф6: окна-оверлея нет, вырезать регион не из чего — заглушка, чтобы не
    // трогать все места вставки. Слой рисуется в главном окне как есть.
    gpui::Empty
}

/// ТОЛЬКО видимость (диалог модалки: ввод уже покрыт input_area на всё окно).
pub fn region_area() -> impl IntoElement {
    gpui::canvas(
        |bounds, window, _cx| {
            push_rect(&REGION_RECTS, &bounds, window.scale_factor());
        },
        |_, _, window, _| {
            apply_window_region(window.scale_factor());
        },
    )
    .absolute()
    .size_full()
}

/// Обрезать overlay-окно до HIT_RECTS (+margin на тени): вне регионов окно
/// физически не существует — НАСТОЯЩАЯ прозрачность. gpui-«Transparent» на
/// Windows не даёт per-pixel альфы (ACCENT-градиент; пустота = ЧЁРНАЯ),
/// LWA_COLORKEY игнорируется flip-model свопчейном — регион единственный
/// надёжный путь.
#[cfg(windows)]
/// Альфа-режим overlay — ДЕФОЛТ: окно на DirectComposition с premultiplied
/// альфой (vendored gpui, per-window env), SetWindowRgn не нужен и только
/// обгрызал углы поповеров. KAMIN_OVERLAY_ALPHA=0 — откат к региону.
pub fn alpha_mode() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("KAMIN_OVERLAY_ALPHA").as_deref() != Ok("0"))
}

pub fn apply_window_region(scale: f32) {
    if alpha_mode() {
        // Альфа рисует прозрачность сама, но СИСТЕМНУЮ РАМКУ окна (Zed
        // 9/1/9/9, неснимаема) надо отрезать: статичный регион = client-
        // область целиком. Ставится один раз на размер (фантомный бордер
        // при драге окна был именно рамкой).
        use windows::Win32::Foundation::HWND;
        use windows::Win32::Graphics::Gdi::{CreateRectRgn, SetWindowRgn};
        static LAST: std::sync::Mutex<(i32, i32, i32, i32)> = std::sync::Mutex::new((0, 0, 0, 0));
        let ovl = OVERLAY_HWND.load(Ordering::Relaxed);
        if ovl == 0 {
            return;
        }
        let overlay = HWND(ovl as *mut _);
        let dx = CLIENT_DX.load(Ordering::Relaxed);
        let dy = CLIENT_DY.load(Ordering::Relaxed);
        let mut cr = windows::Win32::Foundation::RECT::default();
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::GetClientRect(overlay, &mut cr);
        }
        let key = (dx, dy, cr.right, cr.bottom);
        let mut last = LAST.lock().unwrap();
        if *last != key {
            *last = key;
            unsafe {
                let rgn = CreateRectRgn(dx, dy, dx + cr.right, dy + cr.bottom);
                SetWindowRgn(overlay, Some(rgn), true);
            }
        }
        return;
    }
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::SetWindowRgn;
    use windows::Win32::Graphics::Gdi::{CombineRgn, CreateRectRgn, RGN_OR};

    let ovl = OVERLAY_HWND.load(Ordering::Relaxed);
    if ovl == 0 {
        return;
    }
    let overlay = HWND(ovl as *mut _);
    // БЕЗ margin: пустота внутри региона рендерится ЧЁРНЫМ (нет альфы) —
    // тень поповеров жертвуем, скругление повторяем RoundRect-регионом.
    let r = (crate::ui::metrics_radius() * scale) as i32;
    unsafe {
        let acc = CreateRectRgn(0, 0, 0, 0);
        // SetWindowRgn работает в координатах ОКНА; ректы — клиентские
        let dx = CLIENT_DX.load(Ordering::Relaxed);
        let dy = CLIENT_DY.load(Ordering::Relaxed);
        // Кламп к client-области: вырез, налезший на СИСТЕМНУЮ РАМКУ окна,
        // показывает её белым (right/bottom рамки Zed::Window)
        let mut ocr = windows::Win32::Foundation::RECT::default();
        let _ = windows::Win32::UI::WindowsAndMessaging::GetClientRect(overlay, &mut ocr);
        let (max_r, max_b) = (dx + ocr.right - 1, dy + ocr.bottom - 1);
        for [x, y, w, h] in REGION_RECTS.lock().unwrap().iter() {
            // Округление ВНУТРЬ бокса: регион на долю пикселя УЖЕ отрисовки —
            // чёрным полосам по краям неоткуда взяться
            let part = windows::Win32::Graphics::Gdi::CreateRoundRectRgn(
                x.ceil() as i32 + dx + 1,
                y.ceil() as i32 + dy + 1,
                ((x + w).floor() as i32 + dx - 3).min(max_r),
                ((y + h).floor() as i32 + dy - 3).min(max_b),
                r * 2,
                r * 2,
            );
            CombineRgn(Some(acc), Some(acc), Some(part), RGN_OR);
            let _ = windows::Win32::Graphics::Gdi::DeleteObject(part.into());
        }
        // SetWindowRgn забирает владение регионом
        SetWindowRgn(overlay, Some(acc), true);
    }
}

#[cfg(not(windows))]
pub fn apply_window_region(_scale: f32) {}
