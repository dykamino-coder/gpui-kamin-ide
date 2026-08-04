//! Quick Open (Ctrl+P, QuickOpen 1:1): нечёткий поиск файла по индексу
//! воркспейса (kamin:index:findFile). Backdrop rgba(0,0,0,.35), бокс сверху
//! (pt 12vh, w min640), инпут (border-bottom) + список (name + path справа).
//! Enter/клик → открыть файл (OpenFile + files-режим). Esc/бэкдроп закрывают.

use crate::host::events::EdEvent;
use gpui::prelude::*;
use gpui::{AnyElement, Entity, SharedString, div, px};
use gpui_component::Sizable as _;
use gpui_component::input::{Input, InputState};
use gpui_component::scroll::ScrollableElement as _;
use kamin_metrics as m;
use kamin_theme::Palette;

use crate::colors::{rgba, tint};
use crate::host_link::ShellEvent;

const BOX_W: f32 = 640.0;
pub(crate) const MAX_ROWS: usize = 50;

/// Хит поиска файла (rel/abs).
#[derive(Clone)]
pub struct FileHit {
    pub rel: String,
    pub abs: String,
}

fn basename(rel: &str) -> String {
    rel.rsplit('/').next().unwrap_or(rel).to_string()
}
fn dirname(rel: &str) -> String {
    match rel.rfind('/') {
        Some(i) => rel[..i].to_string(),
        None => String::new(),
    }
}

pub(crate) fn hit_row(
    h: &FileHit,
    row_index: usize,
    first: bool,
    light: bool,
    tx: &smol::channel::Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    // `[data-theme=light] .itemActive` — сплошной accent + accent-action-fg
    let base = if first && light {
        rgba(p.accent_primary)
    } else if first {
        tint(rgba(p.accent_primary), 0.14)
    } else {
        gpui::transparent_black().into()
    };
    let (name_color, path_color) = if first && light {
        (
            rgba(p.accent_action_fg),
            tint(rgba(p.accent_action_fg), 0.8),
        )
    } else {
        (rgba(p.text_primary), rgba(p.text_muted))
    };
    let tx = tx.clone();
    let abs = h.abs.clone();
    // `<li role="option">` без `tabIndex` (`QuickOpen.tsx:109`) — не таб-стоп
    div()
        .id(SharedString::from(format!("qo-{}", h.abs)))
        .flex()
        .items_baseline()
        .gap(px(m::SPACE_2))
        .px(px(14.0))
        .py(px(6.0))
        .bg(base)
        .cursor_pointer()
        .on_mouse_move({
            let tx = tx.clone();
            move |_, _, _| {
                // `onMouseEnter` оригинала: подсвечена ровно ОДНА строка —
                // наведение ПЕРЕНОСИТ активную, а не красит вторую поверх
                let _ = tx.try_send(ShellEvent::OverlayRowHover("qo", row_index));
            }
        })
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
            let _ = tx.try_send(ShellEvent::Ed(EdEvent::OpenFile(abs.clone())));
            let _ = tx.try_send(ShellEvent::Ed(EdEvent::SetFileMode("files")));
            let _ = tx.try_send(ShellEvent::CloseQuickOpen);
        })
        .child(
            div()
                .flex_shrink_0()
                .text_size(px(m::FS_SM))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(name_color)
                .child(basename(&h.rel)),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .text_size(px(m::FS_XS))
                .text_color(path_color)
                .text_right()
                .child(dirname(&h.rel)),
        )
        .into_any_element()
}

// Компонент дизайн-системы: аргументы — его пропсы.
#[allow(clippy::too_many_arguments)]
/// Рендер Quick Open. `input` — Entity<InputState> (в root.rs).
pub fn quick_open(
    results: &[FileHit],
    active: usize,
    light: bool,
    // Текст запроса: пустой не показывает «No matches»
    query: &str,
    input: &Entity<InputState>,
    viewport_w: f32,
    viewport_h: f32,
    tx: &smol::channel::Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    // `--overlay-soft`: тем-зависимый, единый источник
    let scrim = crate::ui::scrim::soft_literal();
    let tx_close = tx.clone();
    let tx_key = tx.clone();
    let active_abs = results.get(active).map(|h| h.abs.clone());

    let mut list = div()
        .flex()
        .flex_col()
        .py(px(m::SPACE_1))
        // .list max-height: min(50vh, 480)
        .max_h(px((0.5 * viewport_h).min(480.0)))
        .overflow_y_scrollbar();
    // Пустой запрос не рисует «No matches» (`QuickOpen.tsx:105`)
    let has_query = !query.trim().is_empty();
    if results.is_empty() && has_query {
        list = list.child(
            div()
                .px(px(14.0))
                .py(px(12.0))
                .text_size(px(m::FS_SM))
                .text_color(rgba(p.text_muted))
                .flex()
                .justify_center()
                .child("No matches"),
        );
    } else {
        for (i, h) in results.iter().take(MAX_ROWS).enumerate() {
            list = list.child(hit_row(h, i, i == active, light, tx, p));
        }
    }

    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .flex()
        .justify_center()
        .items_start()
        .pt(px(0.12 * viewport_h)) // 12vh реального вьюпорта
        .bg(scrim)
        .child(crate::overlay::input_area())
        .on_key_down(
            move |ev: &gpui::KeyDownEvent, _, _| match ev.keystroke.key.as_str() {
                "escape" => {
                    let _ = tx_key.try_send(ShellEvent::CloseQuickOpen);
                }
                "enter" => {
                    if let Some(abs) = &active_abs {
                        let _ = tx_key.try_send(ShellEvent::Ed(EdEvent::OpenFile(abs.clone())));
                        let _ = tx_key.try_send(ShellEvent::Ed(EdEvent::SetFileMode("files")));
                        let _ = tx_key.try_send(ShellEvent::CloseQuickOpen);
                    }
                }
                _ => {}
            },
        )
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
            let _ = tx_close.try_send(ShellEvent::CloseQuickOpen);
        })
        .child(
            div()
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .w(px(BOX_W))
                // `min(640, 100vw − 32)` — пола у оригинала нет
                .max_w(px(viewport_w - 32.0))
                .flex()
                .flex_col()
                .overflow_hidden()
                .rounded(px(m::RADIUS_MD))
                .relative()
                .child(crate::probe::registry::probe_area("ov-quickopen"))
                .bg(rgba(p.bg_mantle))
                .child(crate::overlay::hit_area())
                .border_1()
                .border_color(tint(rgba(p.bg_surface), 0.6))
                .shadow(crate::ui::shadows::dropdown())
                .child(
                    div()
                        .w_full()
                        .px(px(14.0))
                        // Оригинал: ряд 40 лог. px. `Input` несёт собственную
                        // высоту, поэтому фиксируем ряд, а не паддинг
                        // (замер цикла 5: было 55-56 против 39-40).
                        .h(px(40.0))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .border_b_1()
                        .border_color(tint(rgba(p.bg_surface), 0.5))
                        .child(
                            Input::new(input)
                                .appearance(false)
                                // `--fs-md` 13 и НУЛЕВОЙ собственный бокс: свои
                                // `px 8 / py 2 / h 24` Input ставит до
                                // `refine_style`, отступы даёт ряд (ревью ц.20)
                                .with_size(gpui_component::Size::Size(px(m::FS_MD / 0.875)))
                                .px_0()
                                .py_0()
                                .h_full(),
                        ),
                )
                .child(list),
        )
        .into_any_element()
}
