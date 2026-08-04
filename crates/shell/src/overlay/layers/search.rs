//! Оверлеи поиска: палитра, QuickOpen, Find in Files, символы.
//!
//! Слой вынесен из `OverlayWindow::render` как есть (`plan/100-refactor-250.md`).

use crate::host_link::ShellEvent;
use crate::root::RootView;
use gpui::prelude::*;

use gpui::Div;

// Компонент дизайн-системы: аргументы — его пропсы.
#[allow(clippy::too_many_arguments)]
pub(crate) fn add_search(
    mut layer: Div,
    r: &RootView,
    p: &'static kamin_theme::Palette,
    tx: &smol::channel::Sender<ShellEvent>,
    vw: f32,
    vh: f32,
    _window: &mut gpui::Window,
    cx: &gpui::App,
) -> Div {
    if r.palette_open
        && let Some(input) = r.palette_input.clone()
    {
        let q = input.read(cx).value().to_string();
        let filtered = crate::ui::command_palette::filter_gated(&r.commands, &q, &r.palette_gate());
        layer = layer.child(crate::ui::command_palette::command_palette(
            &filtered, &q, &input, vw, vh, tx, p,
        ));
    }
    if r.sov.quickopen_open
        && let Some(input) = r.sov.quickopen_input.clone()
    {
        layer = layer.child(crate::ui::quick_open::quick_open(
            &r.sov.quickopen_results,
            r.qo_active_idx(),
            r.theme == kamin_theme::ThemeKind::Light,
            input.read(cx).value().as_ref(),
            &input,
            vw,
            vh,
            tx,
            p,
        ));
    }
    if r.sov.fif_open
        && let Some(input) = r.sov.fif_input.clone()
    {
        layer = layer.child(crate::ui::find_in_files::find_in_files(
            &r.sov.fif_results,
            r.fif_active_idx(),
            r.sov.fif_query_len,
            r.sov.fif_busy,
            &input,
            vw,
            vh,
            tx,
            p,
        ));
    }
    if r.sov.ws_open
        && let Some(input) = r.sov.ws_input.clone()
    {
        layer = layer.child(crate::ui::workspace_symbols::workspace_symbols(
            &r.sov.ws_results,
            r.ws_active_idx(),
            r.theme == kamin_theme::ThemeKind::Light,
            r.sov.ws_query_len,
            &input,
            vw,
            vh,
            tx,
            p,
        ));
    }
    layer
}
