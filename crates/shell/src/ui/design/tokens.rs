//! Токены дизайн-системы: имя, значение, образец цвета, подпись группы.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::rgba;
use crate::colors::tint;
use crate::ui::design_panel::MONO;
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::{Color, Palette};

/// .tokenName: mono fs-xs text-muted (фикс-колонка 90 в рядах).
pub(crate) fn token_name(name: String, w: Option<f32>, p: &Palette) -> AnyElement {
    let mut d = div()
        .font_family(MONO)
        .text_size(px(m::FS_XS))
        .text_color(rgba(p.text_muted))
        .child(SharedString::from(name));
    if let Some(w) = w {
        d = d.w(px(w)).flex_shrink_0();
    }
    d.into_any_element()
}
/// .tokenValue: mono fs-xs text-disabled (фикс-колонка 60).
pub(crate) fn token_value(v: &'static str, p: &Palette) -> AnyElement {
    token_value_w(v, Some(60.0), p)
}
/// То же, но с явной шириной колонки: в сетке радиусов у оригинала это
/// обычный `<span>` по контенту, и жёсткие 60 сдвигали «4px» влево от центра
/// ячейки (ревью ц.13).
pub(crate) fn token_value_w(v: &'static str, w: Option<f32>, p: &Palette) -> AnyElement {
    div()
        .when_some(w, |d, w| d.w(px(w)))
        .flex_shrink_0()
        .font_family(MONO)
        .text_size(px(m::FS_XS))
        .text_color(rgba(p.text_disabled))
        .child(v)
        .into_any_element()
}
/// .swatch: p8 + bg-surface@30% + radius-xs; чип 28×28 + mono-имя.
pub(crate) fn swatch(name: &'static str, c: Color, p: &Palette) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(m::SPACE_2))
        // ширину даёт дорожка грида (`minmax(180px, 1fr)`), а не сам свотч
        .min_w(px(0.))
        .p(px(m::SPACE_2))
        .rounded(px(m::RADIUS_XS))
        .bg(tint(rgba(p.bg_surface), 0.3))
        .child(
            div()
                .w(px(28.0))
                .h(px(28.0))
                .flex_shrink_0()
                .rounded(px(m::RADIUS_XS))
                .border_1()
                .border_color(tint(rgba(p.text_primary), 0.12))
                .bg(rgba(c)),
        )
        .child(
            div()
                .font_family(MONO)
                .text_size(px(m::FS_XS))
                .text_color(rgba(p.text_secondary))
                .child(SharedString::from(format!("--{name}"))),
        )
        .into_any_element()
}
/// .groupLabel: fs-xs uppercase ls .06em text-muted.
pub(crate) fn group_label(label: &'static str, p: &Palette) -> AnyElement {
    div()
        .text_size(px(m::FS_XS))
        .letter_spacing(px(m::FS_XS * 0.06))
        // `.groupLabel` — это `<h3>` без `font-weight` в CSS, значит работает
        // UA-дефолт 700 (ревью ц.6: на кропах оригинала подпись жирная)
        .font_weight(gpui::FontWeight::BOLD)
        .text_color(rgba(p.text_muted))
        .child(label.to_uppercase())
        .into_any_element()
}
