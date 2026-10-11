//! Индикаторы формы и ползунок, отдельно от текстовых полей.

use super::{ACCENT, BORDER, FIELD_BG};
use crate::dom::Element;
use crate::style::apply::apply;
use crate::style::computed::Computed;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px, rgb};

/// Ползунок: дорожка и заполненная часть по значению.
pub(super) fn range(e: &Element, style: &Computed) -> AnyElement {
    let num = |name: &str, default: f32| -> f32 {
        e.attr(name).and_then(|v| v.parse().ok()).unwrap_or(default)
    };
    let (min, max, val) = (num("min", 0.), num("max", 100.), num("value", 50.));
    let accent: gpui::Hsla = super::control_accent(style)
        .map(|c| c.to_hsla())
        .unwrap_or_else(|| rgb(ACCENT).into());
    let frac = if max > min {
        ((val - min) / (max - min)).clamp(0., 1.)
    } else {
        0.
    };
    apply(div(), style)
        .h(px(16.))
        .min_w(px(60.))
        .flex()
        .items_center()
        .child(
            div()
                .relative()
                .w_full()
                .h(px(4.))
                .rounded(px(2.))
                .bg(rgb(BORDER))
                .child(
                    div()
                        .absolute()
                        .left(px(0.))
                        .top(px(0.))
                        .h(px(4.))
                        .w(gpui::relative(frac))
                        .rounded(px(2.))
                        .bg(accent),
                ),
        )
        .into_any_element()
}

pub(super) fn color_swatch(e: &Element, style: &Computed) -> AnyElement {
    let color = e
        .attr("value")
        .and_then(crate::style::values::value::Color::parse)
        .map(|c| c.to_hsla());
    let mut d = apply(div(), style)
        .w(px(28.))
        .h(px(16.))
        .rounded(px(3.))
        .border_1()
        .border_color(rgb(BORDER));
    if let Some(c) = color {
        d = d.bg(c);
    }
    d.into_any_element()
}

/// Полоса выполнения: `<progress value max>`.
pub(super) fn progress(e: &Element, style: &Computed) -> AnyElement {
    let num = |name: &str, default: f32| -> f32 {
        e.attr(name).and_then(|v| v.parse().ok()).unwrap_or(default)
    };
    let frac = (num("value", 0.) / num("max", 1.).max(0.0001)).clamp(0., 1.);
    // Заполненную часть полосы `accent-color` красит так же, как флажок.
    let accent: gpui::Hsla = super::control_accent(style)
        .map(|c| c.to_hsla())
        .unwrap_or_else(|| rgb(ACCENT).into());
    apply(div(), style)
        .h(px(8.))
        .w_full()
        // Внутри колонки шириной по содержимому `w_full` даёт ноль: у полосы
        // нет собственной ширины, и колонка схлопывается вместе с ней.
        .min_w(px(60.))
        .rounded(px(4.))
        .bg(rgb(FIELD_BG))
        .border_1()
        .border_color(rgb(BORDER))
        .child(
            div()
                .h_full()
                .w(gpui::relative(frac))
                .rounded(px(4.))
                .bg(accent),
        )
        .into_any_element()
}
