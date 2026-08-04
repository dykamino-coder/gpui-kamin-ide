//! Страница токенов: цвета, типографика, отступы, радиусы, тени.
//!
//! Блоки перенесены из `design_panel` как есть (`plan/100-refactor-250.md`).

use crate::colors::rgba;
use crate::colors::tint;
use crate::ui::design::tokens::{group_label, swatch, token_name, token_value, token_value_w};
use crate::ui::design_panel::MONO;
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::{Color, Palette};

/// Возвращает готовые секции в порядке оригинала.
pub(crate) fn token_sections(
    p: &Palette,
) -> (AnyElement, AnyElement, AnyElement, AnyElement, AnyElement) {
    // ── Colors: 4 группы как COLOR_GROUPS design-sections.tsx
    let groups: [(&str, Vec<(&str, Color)>); 4] = [
        (
            "Surface",
            vec![
                ("bg-primary", p.bg_primary),
                ("bg-base", p.bg_base),
                ("bg-mantle", p.bg_mantle),
                ("bg-sidebar", p.bg_sidebar),
                ("bg-surface", p.bg_surface),
                ("bg-overlay", p.bg_overlay),
            ],
        ),
        (
            "Text",
            vec![
                ("text-primary", p.text_primary),
                ("text-subtext", p.text_subtext),
                ("text-secondary", p.text_secondary),
                ("text-muted", p.text_muted),
                ("text-disabled", p.text_disabled),
            ],
        ),
        (
            "Accent",
            vec![
                ("accent-blue", p.accent_blue),
                ("accent-sapphire", p.accent_sapphire),
                ("accent-teal", p.accent_teal),
                ("accent-green", p.accent_green),
                ("accent-yellow", p.accent_yellow),
                ("accent-orange", p.accent_orange),
                ("accent-red", p.accent_red),
                ("accent-maroon", p.accent_maroon),
                ("accent-pink", p.accent_pink),
                ("accent-purple", p.accent_purple),
                ("accent-rosewater", p.accent_rosewater),
            ],
        ),
        (
            "Semantic",
            vec![
                ("accent-primary", p.accent_primary),
                ("accent-action", p.accent_action),
                ("accent-action-hover", p.accent_action_hover),
                ("accent-action-fg", p.accent_action_fg),
            ],
        ),
    ];
    let mut colors = div().flex().flex_col().gap(px(m::SPACE_4));
    for (label, tokens) in groups {
        let mut swatches = div().grid().grid_cols_min(px(180.0)).gap(px(m::SPACE_2));
        for (name, c) in tokens {
            swatches = swatches.child(swatch(name, c, p));
        }
        colors = colors.child(
            div()
                .flex()
                .flex_col()
                .gap(px(m::SPACE_2))
                .child(group_label(label, p))
                .child(swatches),
        );
    }

    // ── Typography: font-сэмплы + шкала (.typoStack/.typoScale/.typoRow)
    let font_sample = |token: &'static str, family: &'static str, size: f32| {
        div()
            .flex()
            .flex_col()
            .gap(px(2.0))
            .child(token_name(format!("--{token}"), None, p))
            .child(
                div()
                    .font_family(family)
                    .text_size(px(size))
                    .text_color(rgba(p.text_primary))
                    .child(SharedString::from(format!(
                        "{family} — quick brown fox 0123456789"
                    ))),
            )
    };
    let mut typo_scale = div()
        .flex()
        .flex_col()
        .gap(px(m::SPACE_2))
        .mt(px(m::SPACE_2))
        .pt(px(m::SPACE_3))
        .border_t_1()
        .border_color(tint(rgba(p.bg_surface), 0.5));
    for (name, value, size) in [
        ("fs-xs", "11px", m::FS_XS),
        ("fs-sm", "12px", m::FS_SM),
        ("fs-md", "13px", m::FS_MD),
        ("fs-lg", "16px", m::FS_LG),
        ("fs-xl", "22px", m::FS_XL),
    ] {
        typo_scale = typo_scale.child(
            // .typoRow: grid 90px 60px 1fr, baseline
            div()
                .flex()
                .items_baseline()
                .gap(px(m::SPACE_3))
                .child(token_name(format!("--{name}"), Some(90.0), p))
                .child(token_value(value, p))
                .child(
                    div()
                        .text_size(px(size))
                        .text_color(rgba(p.text_primary))
                        .child("The five steps"),
                ),
        );
    }
    let typo = div()
        .flex()
        .flex_col()
        .gap(px(m::SPACE_3))
        .child(font_sample("font-sans", "Bricolage Grotesque", m::FS_LG))
        .child(font_sample("font-mono", MONO, m::FS_MD))
        .child(typo_scale);

    // ── Spacing: 7 вертикальных рядов (.spaceRow, бар h16 width=токен)
    let mut spacing = div().flex().flex_col().gap(px(m::SPACE_2));
    for (i, s) in [
        m::SPACE_1,
        m::SPACE_2,
        m::SPACE_3,
        m::SPACE_4,
        m::SPACE_5,
        m::SPACE_6,
        m::SPACE_7,
    ]
    .iter()
    .enumerate()
    {
        spacing = spacing.child(
            div()
                .flex()
                .items_center()
                .gap(px(m::SPACE_3))
                .child(token_name(format!("--space-{}", i + 1), Some(90.0), p))
                .child(token_value(
                    ["4px", "8px", "12px", "16px", "20px", "24px", "28px"][i],
                    p,
                ))
                .child(
                    div()
                        .w(px(*s))
                        .h(px(16.0))
                        .rounded(px(m::RADIUS_XS))
                        .bg(rgba(p.accent_primary)),
                ),
        );
    }

    // ── Radius: боксы 80×80 bg-surface + accent 50%-рамка (.radiusItem)
    let mut radius = div().grid().grid_cols_min(px(120.0)).gap(px(m::SPACE_3));
    for (name, value, r) in [
        ("radius-xs", "4px", m::RADIUS_XS),
        ("radius-sm", "8px", m::RADIUS_SM),
        ("radius-md", "12px", m::RADIUS_MD),
        ("radius-lg", "16px", m::RADIUS_LG),
    ] {
        radius = radius.child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(m::SPACE_1))
                .child(
                    div()
                        .w(px(80.0))
                        .h(px(80.0))
                        .rounded(px(r))
                        .border_1()
                        .border_color(tint(rgba(p.accent_primary), 0.5))
                        .bg(rgba(p.bg_surface)),
                )
                .child(token_name(format!("--{name}"), None, p))
                .child(token_value_w(value, None, p)),
        );
    }

    // ── Shadows: все 9 токенов из словаря (SHADOW_TOKENS-порядок),
    // бокс 100×64 r-sm bg-primary (.shadowBox)
    let shadow_tokens: [(&str, Vec<gpui::BoxShadow>); 9] = [
        ("shadow-mini", crate::ui::shadows::mini()),
        ("shadow-card", crate::ui::shadows::card()),
        ("shadow-bar", crate::ui::shadows::bar()),
        ("shadow-tab", crate::ui::shadows::tab()),
        ("shadow-dropdown", crate::ui::shadows::dropdown()),
        ("shadow-card-popup", crate::ui::shadows::card_popup()),
        ("shadow-toast", crate::ui::shadows::toast()),
        ("shadow-lg", crate::ui::shadows::lg()),
        ("shadow-modal", crate::ui::shadows::modal()),
    ];
    let mut shadows = div().grid().grid_cols_min(px(140.0)).gap(px(m::SPACE_4));
    for (name, sh) in shadow_tokens {
        shadows = shadows.child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(m::SPACE_2))
                .child(
                    div()
                        .w(px(100.0))
                        .h(px(64.0))
                        .rounded(px(m::RADIUS_SM))
                        .bg(rgba(p.bg_primary))
                        .shadow(sh),
                )
                .child(token_name(format!("--{name}"), None, p)),
        );
    }

    (
        colors.into_any_element(),
        typo.into_any_element(),
        spacing.into_any_element(),
        radius.into_any_element(),
        shadows.into_any_element(),
    )
}
