//! Кнопки семплов: виды, отрисовка, ряд кнопок.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::rgba;
use gpui::prelude::*;
use gpui::{AnyElement, Div, Stateful, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

/// Вид кнопки из `design-sections.module.css`.
#[derive(Clone, Copy, PartialEq)]
pub enum DsBtn {
    Primary,
    Secondary,
    Danger,
    Ghost,
}
/// Кнопки семплов 1:1: padding 4/16, radius-sm 8, fs-sm 12.
/// Primary — accent-action + 600; Secondary — рамка bg-overlay, hover bg-surface;
/// Danger — accent-red/bg-primary + 600, hover accent-maroon; Ghost —
/// прозрачная рамка (иначе кнопка на 2px уже соседей), hover bg-surface.
pub fn ds_btn(kind: DsBtn, id: &'static str, label: &'static str, p: &Palette) -> Stateful<Div> {
    let (bg, fg, border, hover_bg, hover_fg, bold) = match kind {
        DsBtn::Primary => (
            Some(rgba(p.accent_action)),
            rgba(p.accent_action_fg),
            None,
            rgba(p.accent_action_hover),
            None,
            true,
        ),
        DsBtn::Secondary => (
            None,
            rgba(p.text_primary),
            Some(rgba(p.bg_overlay)),
            rgba(p.bg_surface),
            None,
            false,
        ),
        DsBtn::Danger => (
            Some(rgba(p.accent_red)),
            rgba(p.bg_primary),
            None,
            rgba(p.accent_maroon),
            None,
            true,
        ),
        DsBtn::Ghost => (
            None,
            rgba(p.text_secondary),
            Some(gpui::Rgba {
                r: 0.,
                g: 0.,
                b: 0.,
                a: 0.,
            }),
            rgba(p.bg_surface),
            Some(rgba(p.text_primary)),
            false,
        ),
    };
    let mut b = div()
        .id(id)
        .px(px(m::SPACE_4))
        .py(px(m::SPACE_1))
        .rounded(px(m::RADIUS_SM))
        .text_size(px(m::FS_SM))
        .text_color(fg)
        .cursor_pointer()
        .hover(move |s| {
            let s = s.bg(hover_bg);
            match hover_fg {
                Some(c) => s.text_color(c),
                None => s,
            }
        })
        // Пустая подпись — не ребёнок: иначе в flex-ряду с gap 8 появляется
        // лишний нулевой элемент и кнопка шире оригинала (ревью ц.7)
        .when(!label.is_empty(), |b| b.child(label));
    if let Some(bg) = bg {
        b = b.bg(bg);
    }
    if let Some(bc) = border {
        b = b.border_1().border_color(bc);
    }
    if bold {
        b = b.font_weight(gpui::FontWeight::SEMIBOLD);
    }
    // Кнопки семплов — `<button>` у оригинала, значит таб-стопы с
    // `button:focus-visible` (`theme/global.css:38-43`), ревью ц.26
    crate::ui::focus_ring::focusable(b, id, m::RADIUS_SM, rgba(p.accent_primary))
}
/// Ряд кнопок Primary/Secondary/Danger/Ghost (`ButtonsRow`).
pub fn sample_buttons(p: &Palette) -> AnyElement {
    div()
        .flex()
        .flex_wrap()
        .gap(px(m::SPACE_2))
        .child(ds_btn(DsBtn::Primary, "ds-btn-primary", "Primary", p))
        .child(ds_btn(DsBtn::Secondary, "ds-btn-secondary", "Secondary", p))
        .child(ds_btn(DsBtn::Danger, "ds-btn-danger", "Danger", p))
        .child(ds_btn(DsBtn::Ghost, "ds-btn-ghost", "Ghost", p))
        .into_any_element()
}
/// `.dropdownGroupLabel` / `.menuLabel`: 4/12, fs 11, uppercase, text-muted,
/// трекинг .04em (вендорный патч, план 99).
pub(crate) fn menu_group_label(text: &'static str, p: &Palette) -> Div {
    div()
        .px(px(m::SPACE_3))
        .py(px(m::SPACE_1))
        .text_size(px(m::FS_XS))
        .letter_spacing(px(m::FS_XS * 0.04))
        .text_color(rgba(p.text_muted))
        .child(text.to_uppercase())
}
