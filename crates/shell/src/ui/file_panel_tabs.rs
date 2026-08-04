//! Переключатель Files | Web верха файловой панели (FilePanelModeTabs 1:1):
//! две сшитые вкладки (левая круглит слева, правая справа, шов без бордера),
//! h24 pad 0/10 gap5, codicon files/globe + label; active = accent-градиент
//! 90° 26→14% + бордер 45% + primary-текст. Переключает filePanelMode (persist).

use crate::host::events::EdEvent;
use gpui::prelude::*;
use gpui::{AnyElement, div, linear_color_stop, linear_gradient, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

use crate::colors::{rgba, tint};
use crate::host_link::ShellEvent;
use crate::ui::icon::codicon;

const FILES: &str = "\u{eaf0}";
const GLOBE: &str = "\u{eb01}";

#[allow(clippy::too_many_arguments)]
pub(crate) fn tab(
    id: &'static str,
    glyph: &'static str,
    label: &'static str,
    mode: &'static str,
    active: bool,
    left: bool,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    let divider = tint(rgba(p.text_primary), 0.06); // --divider-soft
    let mut btn = div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(5.0))
        .h(px(24.0))
        .px(px(10.0))
        .border_1()
        .border_color(divider)
        .bg(rgba(p.bg_surface))
        .text_size(px(m::FS_SM))
        .text_color(rgba(p.text_secondary))
        .cursor_pointer();
    // Скруглить только внешнюю кромку; шов (внутренняя граница) без бордера
    if left {
        btn = btn.rounded_l(px(m::RADIUS_MD)).border_r_0();
    } else {
        btn = btn.rounded_r(px(m::RADIUS_MD));
    }
    if active {
        btn = btn
            .bg(linear_gradient(
                90.,
                linear_color_stop(tint(rgba(p.accent_primary), 0.26), 0.0),
                linear_color_stop(tint(rgba(p.accent_primary), 0.14), 1.0),
            ))
            .border_color(tint(rgba(p.accent_primary), 0.45))
            .text_color(rgba(p.text_primary));
    } else {
        let hf = rgba(p.text_primary);
        btn = btn.hover(move |s| s.text_color(hf));
    }
    let tx = tx.clone();
    btn.on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
        let _ = tx.try_send(ShellEvent::Ed(EdEvent::SetFileMode(mode)));
    })
    .child(codicon(glyph, 16.0)) // базовые 16 (ревью ц.1)
    .child(label)
    .into_any_element()
}

/// Files|Web переключатель (сшитые вкладки).
pub fn file_panel_mode_tabs(mode: &str, tx: &Sender<ShellEvent>, p: &Palette) -> AnyElement {
    div()
        .relative()
        .child(crate::probe::registry::probe_area("file-mode-tabs"))
        .flex()
        .flex_shrink_0()
        .child(tab(
            "fpm-files",
            FILES,
            "Files",
            "files",
            mode == "files",
            true,
            tx,
            p,
        ))
        .child(tab(
            "fpm-web",
            GLOBE,
            "Web",
            "web",
            mode == "web",
            false,
            tx,
            p,
        ))
        .into_any_element()
}
