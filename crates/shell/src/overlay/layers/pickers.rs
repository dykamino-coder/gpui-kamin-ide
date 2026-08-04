//! QuickPick и выбор тула для слота.
//!
//! Слой вынесен из `OverlayWindow::render` как есть (`plan/100-refactor-250.md`).

use crate::host_link::ShellEvent;
use crate::root::RootView;
use gpui::prelude::*;

use gpui::Div;

// Компонент дизайн-системы: аргументы — его пропсы.
#[allow(clippy::too_many_arguments)]
pub(crate) fn add_pickers(
    mut layer: Div,
    r: &RootView,
    p: &'static kamin_theme::Palette,
    tx: &smol::channel::Sender<ShellEvent>,
    vw: f32,
    vh: f32,
    window: &mut gpui::Window,
    cx: &gpui::App,
) -> Div {
    if let Some(qp) = r.quick_pick.as_ref() {
        let filter = r
            .qp_input
            .as_ref()
            .map(|i| i.read(cx).value().to_string())
            .unwrap_or_default();
        layer = layer.child(crate::ui::quick_pick::quick_pick(
            qp,
            r.qp_input.as_ref(),
            r.qp_input.as_ref().is_some_and(|inp| {
                gpui::Focusable::focus_handle(inp.read(cx), cx).is_focused(window)
            }),
            &filter,
            vw,
            vh,
            tx,
            p,
        ));
    }

    if let Some((slot, x, y, up)) = r.tool_picker {
        layer = layer.child(crate::ui::tool_picker::tool_picker(
            slot,
            x,
            y,
            up,
            &r.activity,
            tx,
            vw,
            vh,
            window,
            p,
        ));
    }
    layer
}
