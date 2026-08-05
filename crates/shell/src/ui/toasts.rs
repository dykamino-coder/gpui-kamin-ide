//! Тост-стек (Toasts 1:1): фиксирован снизу-справа (bottom 36, right space-4),
//! flex-col gap space-2, max-w 360. Карточка — полупрозрачный bg-surface 50%,
//! border bg-surface 70%, r-md, shadow; иконка severity (цвет = severity),
//! title (600) + message (text-secondary) + action-чипы + dismiss (×).
//! Severity: info=blue, success=green, warning=yellow, error=red.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

use gpui::prelude::*;
use gpui::{Animation, AnimationExt, AnyElement, SharedString, div, px, relative};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

use crate::colors::{rgba, tint};
use crate::host_link::ShellEvent;
use crate::ui::icon::codicon;

/// Каунтдаун не-sticky тоста (оригинал: 8s, пауза на ховере).
pub const TOAST_MS: u32 = 8000;
/// Длительность slide-in/out; после slide-out `ToastGone` удаляет карту.
pub const TOAST_OUT_MS: u64 = 180;

/// Живой таймер тоста. Живёт в `RootView::toast_timers` (не в самом Toast —
/// у Toast 18 мест конструирования). Wall-clock: полоса каунтдауна читает
/// active_ms() на КАЖДЫЙ кадр (насос — repeat-анимация на полосе), ховер
/// держит паузу, `closing` запускает slide-out; фоновый поток обработчика
/// `ShellEvent::Toast` лишь проверяет истечение раз в 100мс.
pub struct ToastTimer {
    started: std::time::Instant,
    /// Сумма мс ЗАВЕРШЁННЫХ ховер-пауз.
    paused_ms: AtomicU32,
    /// Начало текущего ховера, мс от started; u32::MAX = ховера нет.
    hover_since_ms: AtomicU32,
    pub closing: AtomicBool,
}

impl Default for ToastTimer {
    fn default() -> Self {
        Self {
            started: std::time::Instant::now(),
            paused_ms: AtomicU32::new(0),
            hover_since_ms: AtomicU32::new(u32::MAX),
            closing: AtomicBool::new(false),
        }
    }
}

impl ToastTimer {
    /// Прожитые мс БЕЗ ховер-пауз (гладкая, не ступенчатая величина).
    pub fn active_ms(&self) -> u32 {
        let now = self.started.elapsed().as_millis() as u32;
        let hover = self.hover_since_ms.load(Ordering::Relaxed);
        let current_pause = if hover == u32::MAX {
            0
        } else {
            now.saturating_sub(hover)
        };
        now.saturating_sub(self.paused_ms.load(Ordering::Relaxed))
            .saturating_sub(current_pause)
    }

    pub fn set_hovered(&self, hovered: bool) {
        let now = self.started.elapsed().as_millis() as u32;
        if hovered {
            let _ = self.hover_since_ms.compare_exchange(
                u32::MAX,
                now,
                Ordering::Relaxed,
                Ordering::Relaxed,
            );
        } else {
            let since = self.hover_since_ms.swap(u32::MAX, Ordering::Relaxed);
            if since != u32::MAX {
                self.paused_ms
                    .fetch_add(now.saturating_sub(since), Ordering::Relaxed);
            }
        }
    }
}

const INFO: &str = "\u{ea74}";
const PASS: &str = "\u{eba4}";
pub(crate) const WARNING: &str = "\u{ea6c}";
const ERROR: &str = "\u{ea87}";
const CLOSE: &str = "\u{ea76}";

/// Тост (аналог Notification) в стеке RootView.
#[derive(Clone)]
pub struct Toast {
    pub id: String,
    pub severity: String, // info|success|warning|error
    pub title: Option<String>,
    pub message: String,
    pub actions: Vec<String>,
    pub sticky: bool,
}

fn severity_glyph(sev: &str) -> &'static str {
    match sev {
        "success" => PASS,
        "warning" => WARNING,
        "error" => ERROR,
        _ => INFO,
    }
}

fn severity_color(sev: &str, p: &Palette) -> gpui::Rgba {
    match sev {
        "success" => rgba(p.accent_green),
        "warning" => rgba(p.accent_yellow),
        "error" => rgba(p.accent_red),
        _ => rgba(p.accent_blue),
    }
}

fn toast_card(
    t: &Toast,
    timer: Option<&Arc<ToastTimer>>,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    // content: опц. title + message + action-чипы
    let mut content = div().flex_1().min_w(px(0.)).flex().flex_col();
    if let Some(title) = &t.title {
        content = content.child(
            div()
                .mb(px(2.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(rgba(p.text_primary))
                .child(title.clone()),
        );
    }
    content = content.child(
        div()
            .text_color(rgba(p.text_secondary))
            .child(t.message.clone()),
    );
    if !t.actions.is_empty() {
        let mut actions = div()
            .flex()
            .flex_wrap()
            .gap(px(m::SPACE_2))
            .mt(px(m::SPACE_2));
        for label in &t.actions {
            let hover = tint(rgba(p.accent_primary), 0.14);
            let tx = tx.clone();
            let id = t.id.clone();
            actions = actions.child(
                div()
                    .id(SharedString::from(format!("toast-act-{}-{label}", t.id)))
                    .px(px(m::SPACE_3))
                    .py(px(2.0))
                    .rounded(px(m::RADIUS_XS))
                    .border_1()
                    .border_color(tint(rgba(p.accent_primary), 0.4))
                    .text_size(px(m::FS_XS))
                    .text_color(rgba(p.accent_primary))
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover))
                    .on_mouse_down(gpui::MouseButton::Left, {
                        let label = label.clone();
                        move |_, _, _| {
                            // shell.showMessage-тост (id «shellreq-N») ждёт
                            // ВЫБОР — отвечаем хосту выбранным item
                            if let Some(req) = id.strip_prefix("shellreq-")
                                && let Ok(req_id) = req.parse::<u64>()
                            {
                                let label = label.clone();
                                std::thread::spawn(move || {
                                    if let Some(c) = crate::host_link::client() {
                                        c.respond(req_id, Ok(serde_json::json!(label)));
                                    }
                                });
                            }
                            let _ = tx.try_send(ShellEvent::DismissToast(id.clone()));
                        }
                    })
                    .child(label.clone()),
            );
        }
        content = content.child(actions);
    }

    let tx_dismiss = tx.clone();
    let id = t.id.clone();
    let closing = timer
        .map(|tm| tm.closing.load(Ordering::Relaxed))
        .unwrap_or(false);
    let mut card = div()
        .id(SharedString::from(format!("toast-{}", t.id)))
        // Тост — верхний слой: клики и ховеры по его телу не должны
        // проваливаться в элементы под ним (кликался титлбар сквозь тост).
        .occlude()
        .relative()
        .flex()
        .items_start()
        .gap(px(m::SPACE_3))
        .p(px(m::SPACE_3))
        .px(px(m::SPACE_4))
        .rounded(px(m::RADIUS_MD))
        // Оригинал 1:1: surface 50% + backdrop-filter blur(8). Блюр рисует
        // канвас-ребёнок ниже (KaminIDE patch paint_backdrop_blur в gpui).
        .bg(tint(rgba(p.bg_surface), 0.5))
        .border_1()
        .border_color(tint(rgba(p.bg_surface), 0.7))
        .shadow(crate::ui::shadows::card_popup())
        .text_size(px(m::FS_SM))
        .child(
            // Настоящий backdrop-filter blur(8): рендерный проход в vendored
            // gpui (paint_backdrop_blur). Рисуется ПОСЛЕ фона карты и ДО
            // контента: блюрит содержимое под тостом вместе с полупрозрачным
            // тинтом (линейно эквивалентно blur(фон)+тинт).
            gpui::canvas(
                |_, _, _| {},
                |bounds, _, window, _| {
                    window.paint_backdrop_blur(bounds, gpui::Corners::all(px(m::RADIUS_MD)));
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
        .child(
            // `.icon{font-size:var(--fs-md)}` стоит на самом `.codicon` →
            // проигрывает вендорной базе: эффективный кегль 16 (ревью ц.14)
            codicon(severity_glyph(&t.severity), 16.0)
                .flex_shrink_0()
                .mt(px(2.0))
                .text_color(severity_color(&t.severity, p)),
        )
        .child(content)
        .child(
            div()
                .id(SharedString::from(format!("toast-x-{}", t.id)))
                .flex_shrink_0()
                .w(px(16.0))
                .h(px(16.0))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(m::FS_XS))
                .text_color(rgba(p.text_disabled))
                .cursor_pointer()
                .hover(|s| s.text_color(rgba(p.text_primary)))
                .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
                    // Dismiss ожидающего showMessage-тоста = ответ null хосту
                    if let Some(req) = id.strip_prefix("shellreq-")
                        && let Ok(req_id) = req.parse::<u64>()
                    {
                        std::thread::spawn(move || {
                            if let Some(c) = crate::host_link::client() {
                                c.respond(req_id, Ok(serde_json::Value::Null));
                            }
                        });
                    }
                    let _ = tx_dismiss.try_send(ShellEvent::DismissToast(id.clone()));
                })
                // `.dismiss{font-size:fs-xs}` стоит на КНОПКЕ, а глиф —
                // вложенный span без класса → чистая база 16 (ревью ц.14)
                .child(codicon(CLOSE, 16.0)),
        );

    if let Some(tm) = timer {
        // Ховер ставит каунтдаун на паузу (пишем прямо в Arc — без
        // event-раундтрипа).
        let tm_h = tm.clone();
        card = card.on_hover(move |hovering, _, _| {
            tm_h.set_hovered(*hovering);
        });
        // Каунтдаун-полоса снизу. Ширина считается от wall-clock НА КАЖДЫЙ
        // кадр, а repeat-анимация на полосе — насос репейнтов 60fps, пока
        // тост жив (ступенчатые 10Гц «подтормаживали» — жалоба юзера).
        if !t.sticky && !closing {
            let tm_bar = tm.clone();
            // Трек с боковыми инсетами = RADIUS_MD: полоса живёт в прямом
            // участке нижней кромки и не вылезает за скругление углов
            // (content mask gpui прямоугольный, радиусы не режет).
            card = card.child(
                div()
                    .absolute()
                    .bottom_0()
                    .left(px(m::RADIUS_MD))
                    .right(px(m::RADIUS_MD))
                    .h(px(2.0))
                    .child(
                        div()
                            .h_full()
                            .rounded(px(1.0))
                            .bg(tint(severity_color(&t.severity, p), 0.55))
                            .with_animation(
                                SharedString::from(format!("toast-bar-{}", t.id)),
                                Animation::new(Duration::from_secs(1)).repeat(),
                                move |d, _| {
                                    let active = tm_bar.active_ms().min(TOAST_MS) as f32;
                                    d.w(relative(1.0 - active / TOAST_MS as f32))
                                },
                            ),
                    ),
            );
        }
    }

    // Slide-in справа + fade при появлении; slide-out после Dismiss
    // (двухфазное удаление: closing → анимация → ToastGone). Разные
    // element-id перезапускают анимацию при смене фазы. Сдвиг — mr<0:
    // карта прижата вправо items_end'ом стека, ml её не двигает.
    if closing {
        card.with_animation(
            SharedString::from(format!("toast-out-{}", t.id)),
            Animation::new(Duration::from_millis(TOAST_OUT_MS)).with_easing(gpui::ease_in_out),
            |d, delta| d.mr(px(-delta * 48.0)).opacity(1.0 - delta),
        )
        .into_any_element()
    } else {
        card.with_animation(
            SharedString::from(format!("toast-in-{}", t.id)),
            Animation::new(Duration::from_millis(TOAST_OUT_MS)).with_easing(gpui::ease_out_quint()),
            |d, delta| d.mr(px(-(1.0 - delta) * 48.0)).opacity(delta),
        )
        .into_any_element()
    }
}

/// Стек тостов — фиксирован снизу-справа. Пустой → пустой div.
pub fn toasts(
    items: &[Toast],
    timers: &HashMap<String, Arc<ToastTimer>>,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    // Позицию задаёт overlay-обёртка (absolute у нулевого anchor сжимал
    // карты в интринсик-ширину)
    let mut stack = div()
        .relative()
        .child(crate::probe::registry::probe_area("ov-toasts"))
        .flex()
        .flex_col()
        .gap(px(m::SPACE_2));
    for t in items {
        stack = stack.child(toast_card(t, timers.get(&t.id), tx, p));
    }
    stack.into_any_element()
}
