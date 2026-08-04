//! Каркас панели: секция, блок и блок с подсказкой.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::{rgba, tint};
use gpui::prelude::*;
use gpui::{AnyElement, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

pub(crate) fn section(
    title: &'static str,
    subtitle: &'static str,
    body: AnyElement,
    p: &Palette,
) -> AnyElement {
    // Оригинал DesignPanel.module.css:
    //   .section       { gap: 12 }      .sectionHeader { gap: 2 }
    //   .sectionTitle  { fs-lg 16/600 } .sectionSubtitle { fs-sm 12, lh-snug }
    //   .sectionBody   { border 1px color-mix(bg-surface 60%); radius-md;
    //                    background: bg-mantle; padding: 16 }
    div()
        .flex()
        .flex_col()
        .gap(px(m::SPACE_3))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(px(m::FS_LG))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(rgba(p.text_primary))
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(m::FS_SM))
                        .line_height(px(m::FS_SM * 1.3))
                        .text_color(rgba(p.text_muted))
                        .child(subtitle),
                ),
        )
        .child(
            div()
                .border_1()
                .border_color(tint(rgba(p.bg_surface), 0.6))
                .rounded(px(m::RADIUS_MD))
                .bg(rgba(p.bg_mantle))
                .p(px(m::SPACE_4))
                .child(body),
        )
        .into_any_element()
}
/// `.compRow` + `.compLabel` — блок семпла с подписью.
/// Оригинал: колонка gap 8, подпись fs-xs uppercase text-muted
/// (`letter-spacing .06em` в gpui недоступен).
pub(crate) fn block(label: &'static str, body: AnyElement, p: &Palette) -> AnyElement {
    block_hint(label, None, body, p)
}
/// То же с `.compHint` — пояснением под подписью
/// (fs-xs, line-height snug 1.3, text-muted, отбивка снизу space-1).
pub(crate) fn block_hint(
    label: &'static str,
    hint: Option<&'static str>,
    body: AnyElement,
    p: &Palette,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(m::SPACE_2))
        .child(
            div()
                .text_size(px(m::FS_XS))
                .letter_spacing(px(m::FS_XS * 0.06))
                // `<h3 class=compLabel>` — CSS веса не задаёт, UA-дефолт 700
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(rgba(p.text_muted))
                .child(label.to_uppercase()),
        )
        .when_some(hint, |d, h| {
            d.child(
                div()
                    .mb(px(m::SPACE_1))
                    .text_size(px(m::FS_XS))
                    .line_height(px(m::FS_XS * 1.3))
                    .text_color(rgba(p.text_muted))
                    .child(h),
            )
        })
        // `.compInline` — тело блока всегда flex-wrap-ряд с gap space-2;
        // без него одиночный ребёнок (меню, дерево) растягивался колонкой
        // на всю ширину панели
        .child(div().flex().flex_wrap().gap(px(m::SPACE_2)).child(body))
        .into_any_element()
}
