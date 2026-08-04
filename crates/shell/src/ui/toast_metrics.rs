//! Метрики карточки тоста: отступы, радиус, полоса таймера.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::rgba;
use gpui::prelude::*;
use gpui::{div, px};
use kamin_theme::Palette;

/// `#card` — 12px 14px, gap 6, радиус 10, рамка акцентом.
pub(crate) const CARD_PAD_X: f32 = 14.0;
pub(crate) const CARD_PAD_Y: f32 = 12.0;
pub(crate) const CARD_RADIUS: f32 = 10.0;
pub(crate) const CARD_GAP: f32 = 6.0;
/// `body { padding: 2px }` — зазор от края окна под тень.
pub(crate) const BODY_PAD: f32 = 2.0;
/// `CARD_OPACITY` / `CARD_HOVER_OPACITY` (`toast-card.ts:17-18`).
pub(crate) const CARD_ALPHA: f32 = 0.96;
pub(crate) const CARD_ALPHA_HOVER: f32 = 0.98;
pub(crate) const BAR_H: f32 = 2.0;
pub(crate) const DOT: f32 = 6.0;
/// Кнопка ряда действий: последняя — `.btn.primary`, прочие — `.btn.ghost`.
pub(crate) fn action_btn(
    label: String,
    primary: bool,
    accent: gpui::Rgba,
    p: &Palette,
) -> gpui::Stateful<gpui::Div> {
    let base = div()
        .id(gpui::SharedString::from(format!("toast-act-{label}")))
        .rounded(px(6.0))
        .py(px(6.0))
        .text_size(px(11.0))
        .cursor_pointer()
        .border_1();
    if primary {
        // `.btn.primary { background: accent; color: accent-action-fg; 600 }`
        base.px(px(16.0))
            .bg(accent)
            .border_color(accent)
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(rgba(p.accent_action_fg))
            .child(label)
    } else {
        // `.btn.ghost` + hover: bg-surface, рамка text-disabled, text-primary
        let hover_bg = rgba(p.bg_surface);
        let hover_border = rgba(p.text_disabled);
        let hover_fg = rgba(p.text_primary);
        base.px(px(12.0))
            .border_color(rgba(p.bg_overlay))
            .text_color(rgba(p.text_secondary))
            .hover(move |s| {
                s.bg(hover_bg)
                    .border_color(hover_border)
                    .text_color(hover_fg)
            })
            .child(label)
    }
}
