//! Части панели логов: поле фильтра, кнопки, отбор строк.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::rgba;
use crate::ui::icon::codicon;
use gpui::prelude::*;
use gpui::{AnyElement, Entity, div, px};
use gpui_component::input::{Input, InputState};
use gpui_component::{Sizable as _, Size};
use kamin_metrics as m;
use kamin_theme::Palette;

/// Инпут фильтра в тулбаре (.search: flex-1, 4×8, bg-base, border bg-surface).
pub fn filter_input(input: &Entity<InputState>, focused: bool, p: &Palette) -> AnyElement {
    div()
        .flex()
        .items_center()
        .flex_1()
        .min_w(px(0.))
        // `.search { padding: 4px 8px }` → инсет текста 9 (8 + рамка 1).
        // `Input` добавляет СВОИ 8 px (`input_px`), поэтому обёртке остаётся
        // 1: 1 + 8 = 9. Раньше стояло 8 и суммарно выходило 17 (ревью ц.26)
        .pl(px(1.0))
        .pr(px(m::SPACE_2))
        .py(px(4.0))
        .rounded(px(m::RADIUS_SM))
        .bg(rgba(p.bg_base))
        .border_1()
        // `:focus-within → accent-primary` (ревью ц.23)
        .border_color(if focused {
            rgba(p.accent_primary)
        } else {
            rgba(p.bg_surface)
        })
        .text_size(px(m::FS_SM))
        .text_color(rgba(p.text_primary))
        .child(
            div().flex_1().child(
                Input::new(input)
                    .appearance(false)
                    .with_size(Size::Size(px(m::FS_SM / 0.875))),
            ),
        )
        .into_any_element()
}
/// Кнопка тулбара 26×26 (.toolBtn): hover bg-surface, disabled 0.4.
pub fn tool_btn(
    id: &'static str,
    glyph: &'static str,
    tip: &'static str,
    enabled: bool,
    p: &Palette,
    on_click: impl Fn(&mut gpui::App) + 'static,
) -> AnyElement {
    let mut b = div()
        .id(id)
        .w(px(26.0))
        .h(px(26.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(m::RADIUS_SM))
        .text_color(rgba(p.text_secondary))
        .tooltip(crate::ui::tooltip::tooltip(tip))
        .child(codicon(glyph, 14.0));
    if enabled {
        let hb = rgba(p.bg_surface);
        b = b
            .cursor_pointer()
            .hover(move |s| s.bg(hb).text_color(rgba(p.text_primary)))
            .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| on_click(cx));
    } else {
        // `.toolBtn[disabled] { opacity: .4; cursor: not-allowed }`
        // (`LogsPanel.module.css:97`) — курсор был обычной стрелкой
        // (ревью ц.26)
        b = b
            .opacity(0.4)
            .cursor(gpui::CursorStyle::OperationNotAllowed);
    }
    crate::ui::focus_ring::focusable(
        b,
        &format!("logs:{id}"),
        m::RADIUS_XS,
        rgba(p.accent_primary),
    )
    .into_any_element()
}
/// Матч строки фильтру (case-insensitive; пустой фильтр = всё).
pub fn matches(line: &str, filter: &str) -> bool {
    filter.is_empty() || line.to_lowercase().contains(&filter.to_lowercase())
}
/// «all» → «All» (аналог `text-transform: capitalize`).
pub fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}
