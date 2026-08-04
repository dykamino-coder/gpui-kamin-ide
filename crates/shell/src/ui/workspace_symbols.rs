//! Go to Symbol in Workspace (Ctrl+T, WorkspaceSymbols 1:1): опрашивает
//! workspaceSymbol-провайдеры (kamin:lang:workspaceSymbol). Переиспользует
//! бокс QuickOpen: инпут + список (symbol-kind codicon + name + path справа
//! «container · basename»). Enter/клик → открыть файл. Esc/бэкдроп закрывают.

pub use crate::ui::ws_row::SymbolHit;

use crate::host::events::EdEvent;
use crate::ui::ws_row::symbol_row;
use gpui::prelude::*;
use gpui::{AnyElement, Entity, div, px};
use gpui_component::Sizable as _;
use gpui_component::input::{Input, InputState};
use gpui_component::scroll::ScrollableElement as _;
use kamin_metrics as m;
use kamin_theme::Palette;

use crate::colors::{rgba, tint};
use crate::host_link::ShellEvent;

const BOX_W: f32 = 640.0;
pub(crate) const MAX_ROWS: usize = 100;

// Компонент дизайн-системы: аргументы — его пропсы.
#[allow(clippy::too_many_arguments)]
/// Рендер Go to Symbol. `input` — Entity<InputState> (в root.rs).
pub fn workspace_symbols(
    results: &[SymbolHit],
    active: usize,
    light: bool,
    query_len: usize,
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
    let active_uri = results.get(active).map(|h| (h.uri.clone(), h.line));

    let mut list = div()
        .flex()
        .flex_col()
        .py(px(m::SPACE_1))
        // .list max-height: min(50vh, 480)
        .max_h(px((0.5 * viewport_h).min(480.0)))
        .overflow_y_scrollbar();
    if results.is_empty() && query_len > 0 {
        list = list.child(
            div()
                .px(px(14.0))
                .py(px(12.0))
                .text_size(px(m::FS_SM))
                .text_color(rgba(p.text_muted))
                .flex()
                .justify_center()
                .child("No symbols"),
        );
    } else {
        for (i, h) in results.iter().take(MAX_ROWS).enumerate() {
            list = list.child(symbol_row(i, h, i == active, light, tx, p));
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
                    let _ = tx_key.try_send(ShellEvent::CloseWorkspaceSymbols);
                }
                "enter" => {
                    if let Some((uri, line)) = &active_uri {
                        let _ = tx_key.try_send(match line {
                            Some(l) => ShellEvent::Ed(EdEvent::OpenFileAt(uri.clone(), *l)),
                            None => ShellEvent::Ed(EdEvent::OpenFile(uri.clone())),
                        });
                        let _ = tx_key.try_send(ShellEvent::Ed(EdEvent::SetFileMode("files")));
                        let _ = tx_key.try_send(ShellEvent::CloseWorkspaceSymbols);
                    }
                }
                _ => {}
            },
        )
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
            let _ = tx_close.try_send(ShellEvent::CloseWorkspaceSymbols);
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
                .child(crate::probe::registry::probe_area("ov-symbols"))
                .bg(rgba(p.bg_mantle))
                .child(crate::overlay::hit_area())
                .border_1()
                .border_color(tint(rgba(p.bg_surface), 0.6))
                .shadow(crate::ui::shadows::dropdown())
                .child(
                    div()
                        .w_full()
                        .px(px(14.0))
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
