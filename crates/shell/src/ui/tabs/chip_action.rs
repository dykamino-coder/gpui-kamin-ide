//! Кнопка-действие чипа сессии (disconnect / pin).
//!
//! Вынесено из `chip.rs` без изменения поведения
//! (`plan/100-refactor-250.md`).

use crate::colors::{rgba, tint};
use crate::ui::focus_ring::FocusRing;
use crate::ui::sessions::glyphs::FA_THUMBTACK;
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

pub(crate) const DISCONNECT_GLYPH: &str = "\u{ead0}"; // codicon-debug-disconnect
/// Кнопка-действие чипа (16×16): скрыта, появляется по ховеру чипа.
pub(crate) fn chip_action(
    id: String,
    glyph: &'static str,
    tip: &'static str,
    group: SharedString,
    p: &Palette,
    on_click: impl Fn() + 'static,
    always: bool,
) -> AnyElement {
    // .close 18×18, глиф 10, hover text-primary 14% (ревью ц.1)
    let hover_bg = tint(rgba(p.text_primary), 0.14);
    let is_fa = glyph == FA_THUMBTACK;
    let icon: AnyElement = if is_fa {
        crate::ui::icon::fa(glyph, 10.0).into_any_element()
    } else {
        // Живой прод: .codicon-каскад перебивает .close до 16px
        crate::ui::icon::codicon(glyph, 16.0).into_any_element()
    };
    div()
        .id(SharedString::from(id.clone()))
        .focus_ring(
            &format!("chipact:{id}"),
            m::RADIUS_XS,
            rgba(p.accent_primary),
        )
        .flex_shrink_0()
        .w(px(18.0))
        .h(px(18.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.0))
        .text_color(rgba(p.text_muted))
        .cursor_pointer()
        .when(!always, |d| {
            d.invisible().group_hover(group, |s| s.visible())
        })
        .hover(move |s| s.bg(hover_bg).text_color(rgba(p.text_primary)))
        .tooltip(crate::ui::tooltip::tooltip(tip))
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            on_click();
        })
        .child(icon)
        .into_any_element()
}
