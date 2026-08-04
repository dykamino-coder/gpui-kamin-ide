//! Части панели Problems: имя папки и кнопка-счётчик.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::{rgba, tint};
use crate::host::events::CzEvent;
use crate::host_link::ShellEvent;
use crate::ui::icon::codicon;
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

pub(crate) const FILE_CAP_STEP: usize = 200;
pub(crate) const ROW_CAP: usize = 200;
pub(crate) fn dir_name(path: &str) -> String {
    let p = path.replace('\\', "/");
    match p.rfind('/') {
        Some(i) => p[..i].to_string(),
        None => String::new(),
    }
}
// Компонент дизайн-системы: аргументы — его пропсы.
#[allow(clippy::too_many_arguments)]
/// .countBtn: тумблер severity-фильтра в хедере.
pub(crate) fn count_btn(
    sev: u8,
    glyph: &'static str,
    count: usize,
    active: bool,
    icon_color: gpui::Rgba,
    tip: &'static str,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    let mut b = div()
        .id(SharedString::from(format!("prob-flt-{sev}")))
        .flex()
        .items_center()
        .gap(px(3.0))
        .px(px(6.0))
        .py(px(1.0))
        .rounded(px(9.0))
        .border_1()
        .border_color(tint(rgba(p.text_primary), 0.0))
        .text_size(px(m::FS_XS))
        .text_color(rgba(p.text_muted))
        // `.countBtn .codicon` написан БЕЗ `:global` — в CSS-модуле класс
        // хешируется и с реальным `.codicon` не совпадает, значит действует
        // вендорная база 16 (ревью ц.14)
        .child(codicon(glyph, 16.0).text_color(if count > 0 {
            icon_color
        } else {
            rgba(p.text_muted)
        }))
        .child(format!("{count}"));
    if active {
        b = b
            .bg(tint(rgba(p.accent_primary), 0.18))
            .border_color(tint(rgba(p.accent_primary), 0.4))
            .text_color(rgba(p.text_primary));
    }
    if count > 0 {
        let tx = tx.clone();
        b = b
            .cursor_pointer()
            // `.countBtn:hover:not(:disabled)` (0,3,0) перебивает
            // `.countActive` (0,1,0): ховер работает и у активной пилюли
            .when(true, |b| {
                b.hover({
                    let hb = tint(rgba(p.bg_surface), 0.7);
                    move |s| s.bg(hb)
                })
            })
            .tooltip(crate::ui::tooltip::tooltip(tip))
            .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                cx.stop_propagation();
                let _ = tx.try_send(ShellEvent::Cz(CzEvent::ToggleProblemsFilter(sev)));
            });
    } else {
        b = b.opacity(0.8);
    }
    crate::ui::focus_ring::focusable(
        b,
        &format!("probflt:{sev}"),
        m::RADIUS_XS,
        rgba(p.accent_primary),
    )
    .into_any_element()
}
