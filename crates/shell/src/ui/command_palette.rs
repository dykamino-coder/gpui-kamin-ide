//! Палитра команд (CommandPalette 1:1): скрим overlay-modal, панель по центру
//! сверху (top 84, w640, max-h 60vh, bg-mantle). Строка ввода (search + Input +
//! Esc kbd), список команд (title + category-префикс + id моно справа), футер
//! «N commands · Enter to run». Первый ряд подсвечен; Enter запускает первый.

pub use crate::ui::palette_filter::{CommandItem, filter_gated, palette_gate};
use crate::ui::palette_row::command_row;

use crate::ui::palette_filter::{MAX_ROWS, PALETTE_TOP, PALETTE_W};
use gpui::prelude::*;
use gpui::{AnyElement, Entity, SharedString, div, px};
use gpui_component::Sizable as _;
use gpui_component::input::{Input, InputState};
use gpui_component::scroll::ScrollableElement as _;
use kamin_metrics as m;
use kamin_theme::Palette;

use crate::colors::{rgba, tint};
use crate::host_link::ShellEvent;
use crate::ui::icon::{SEARCH, codicon};

/// Рендер палитры. `input` — Entity<InputState> (создаётся в root.rs).
pub fn command_palette(
    filtered: &[CommandItem],
    // Текст запроса — для empty-состояния с кавычками
    query: &str,
    input: &Entity<InputState>,
    viewport_w: f32,
    viewport_h: f32,
    tx: &smol::channel::Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    // `--overlay-modal`: в светлой теме чернильный, не чёрный
    let scrim = crate::ui::scrim::modal();
    let tx_close = tx.clone();
    let tx_key = tx.clone();
    let first_id = filtered.first().map(|c| c.id.clone());

    let mut list = div()
        .flex_1()
        .min_h(px(0.))
        .flex()
        .flex_col()
        .gap(px(1.0))
        .p(px(m::SPACE_1))
        .overflow_y_scrollbar();
    if filtered.is_empty() {
        list = list.child(
            div()
                .px(px(m::SPACE_4))
                .py(px(m::SPACE_3))
                .italic()
                .text_color(rgba(p.text_muted))
                // `No commands match "{query}"` (`CommandPalette.tsx:61`) —
                // ASCII-кавычки и ВСЕГДА, даже при пустом запросе (ревью ц.17)
                .child(SharedString::from(format!("No commands match \"{query}\""))),
        );
    } else {
        for (i, c) in filtered.iter().take(MAX_ROWS).enumerate() {
            list = list.child(command_row(c, i == 0, tx, p));
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
        .pt(px(PALETTE_TOP))
        .bg(scrim)
        .child(crate::overlay::input_area())
        .on_key_down(
            move |ev: &gpui::KeyDownEvent, _, _| match ev.keystroke.key.as_str() {
                "escape" => {
                    let _ = tx_key.try_send(ShellEvent::ClosePalette);
                }
                "enter" => {
                    if let Some(id) = &first_id {
                        let _ = tx_key.try_send(ShellEvent::RunCommand(id.clone()));
                    }
                }
                _ => {}
            },
        )
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
            let _ = tx_close.try_send(ShellEvent::ClosePalette);
        })
        .child(
            div()
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .w(px(PALETTE_W))
                // `min(640, 100vw − 32)` — пола у оригинала нет
                .max_w(px(viewport_w - 32.0))
                // `--layout-palette-max-height: 60vh` (оригинал), а не доля остатка
                // `.scrim` не задаёт `align-items` ⇒ stretch: панель ВСЕГДА
                // 60vh, а не по содержимому (ревью ц.17).
                // `.scrim` не задаёт `align-items` → stretch, значит фактическая
                // высота = `min(60vh, 100vh − 84)`; в QuickPick формула уже такая,
                // здесь стоял голый 60vh (ревью ц.26)
                .h(px((0.6 * viewport_h).min(viewport_h - 84.0).max(120.0)))
                .flex()
                .flex_col()
                .overflow_hidden()
                .rounded(px(m::RADIUS_MD))
                .relative()
                .child(crate::probe::registry::probe_area("ov-palette"))
                .bg(rgba(p.bg_mantle))
                .child(crate::overlay::hit_area())
                .border_1()
                .border_color(tint(rgba(p.bg_surface), 0.8))
                // `--shadow-modal` из словаря: в светлой теме он ink-tinted
                // .18, а не чёрный .5 (ревью ц.11)
                .shadow(crate::ui::shadows::modal())
                // input-row
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(m::SPACE_2))
                        .px(px(m::SPACE_4))
                        // css: space-3, но gpui-Input несёт собственную высоту
                        // (~30px против 20px строки оригинала) — компенсируем
                        // Замер оригинала: ряд 43.7 лог. — фиксируем,
                        // как в QuickOpen/FiF/symbols (paddings не попадают)
                        .h(px(44.0))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .border_b_1()
                        .border_color(tint(rgba(p.bg_surface), 0.6))
                        .child(codicon(SEARCH, 16.0).text_color(rgba(p.text_muted)))
                        .child(
                            div().flex_1().child(
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
                        .child(
                            div()
                                .font_family("JetBrains Mono")
                                .text_size(px(m::FS_XS))
                                .text_color(rgba(p.text_muted))
                                .bg(tint(rgba(p.bg_overlay), 0.5))
                                .px(px(6.0))
                                .py(px(2.0))
                                .rounded(px(m::RADIUS_XS))
                                .child("Esc"),
                        ),
                )
                .child(list)
                // footer
                .child(
                    div()
                        .px(px(m::SPACE_4))
                        .py(px(m::SPACE_2))
                        .border_t_1()
                        .border_color(tint(rgba(p.bg_surface), 0.6))
                        .text_size(px(m::FS_XS))
                        .text_color(rgba(p.text_muted))
                        .child(format!(
                            "{} command{} · Enter to run",
                            filtered.len(),
                            if filtered.len() == 1 { "" } else { "s" }
                        )),
                ),
        )
        .into_any_element()
}

#[cfg(test)]
mod gate_tests {
    use super::{CommandItem, filter_gated, palette_gate};

    fn cmd(id: &str) -> CommandItem {
        CommandItem {
            id: id.into(),
            title: id.into(),
            category: None,
        }
    }

    #[test]
    fn gate_matches_state_ts() {
        let mut ctx = crate::when::ContextValues::new();
        ctx.insert("isDev".into(), serde_json::json!(true));
        let entries = vec![
            // одна запись без `when` — всегда видна
            ("a.always".to_string(), String::new()),
            // `when` истинен
            ("b.dev".to_string(), "isDev".to_string()),
            // `when` ложен
            ("c.prod".to_string(), "!isDev".to_string()),
            // ДВЕ записи: хватает одной истинной
            ("d.any".to_string(), "!isDev".to_string()),
            ("d.any".to_string(), "isDev".to_string()),
        ];
        let gate = palette_gate(&entries, &ctx);
        let cmds = vec![
            cmd("a.always"),
            cmd("b.dev"),
            cmd("c.prod"),
            cmd("d.any"),
            // записи в палитре нет вовсе — видна по умолчанию
            cmd("e.free"),
        ];
        let ids: Vec<String> = filter_gated(&cmds, "", &gate)
            .into_iter()
            .map(|c| c.id)
            .collect();
        assert_eq!(ids, vec!["a.always", "b.dev", "d.any", "e.free"]);
    }
}
