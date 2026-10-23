//! Индикаторы формы и ползунок, отдельно от текстовых полей.

use super::{SystemColor, native_color};
use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::paint::apply::apply;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px};

/// Ползунок: дорожка и заполненная часть по значению.
pub(super) fn range(
    e: &Element,
    style: &crate::style::computed::BoxStyle,
    vcx: ValueCx<'_>,
) -> AnyElement {
    let num = |name: &str, default: f32| -> f32 {
        e.attr(name).and_then(|v| v.parse().ok()).unwrap_or(default)
    };
    let (min, max, val) = (num("min", 0.), num("max", 100.), num("value", 50.));
    let accent: gpui::Hsla = super::control_accent(style)
        .map(|c| c.to_hsla())
        .unwrap_or_else(|| native_color(style, SystemColor::AccentColor));
    let frac = if max > min {
        ((val - min) / (max - min)).clamp(0., 1.)
    } else {
        0.
    };
    let mut paint = apply(div(), style, vcx).flex().items_center().child(
        div()
            .relative()
            .w_full()
            .h(px(4.))
            .rounded(px(2.))
            .bg(native_color(style, SystemColor::ButtonBorder))
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
    );
    paint.style().sizing_keywords = Some(crate::paint::apply::intrinsic_size::keywords(style));
    super::control(
        paint,
        crate::style::ua::forms::size(crate::style::ua::forms::ControlKind::Range),
    )
}

pub(super) fn color_swatch(
    e: &Element,
    style: &crate::style::computed::BoxStyle,
    vcx: ValueCx<'_>,
) -> AnyElement {
    let color = e
        .attr("value")
        .and_then(|v| crate::style::values::value::Color::parse(v, vcx))
        .map(|c| c.to_hsla());
    let mut d = apply(div(), style, vcx)
        .rounded(px(3.))
        .border_1()
        .border_color(native_color(style, SystemColor::ButtonBorder));
    if let Some(c) = color {
        d = d.bg(c);
    }
    super::control(
        d,
        crate::style::ua::forms::size(crate::style::ua::forms::ControlKind::Color),
    )
}

/// Полоса выполнения: `<progress value max>`.
pub(super) fn progress(
    e: &Element,
    style: &crate::style::computed::BoxStyle,
    vcx: ValueCx<'_>,
) -> AnyElement {
    let num = |name: &str, default: f32| -> f32 {
        e.attr(name).and_then(|v| v.parse().ok()).unwrap_or(default)
    };
    let frac = (num("value", 0.) / num("max", 1.).max(0.0001)).clamp(0., 1.);
    // Заполненную часть полосы `accent-color` красит так же, как флажок.
    let accent: gpui::Hsla = super::control_accent(style)
        .map(|c| c.to_hsla())
        .unwrap_or_else(|| native_color(style, SystemColor::AccentColor));
    apply(div(), style, vcx)
        .h(px(8.))
        .w_full()
        // Внутри колонки шириной по содержимому `w_full` даёт ноль: у полосы
        // нет собственной ширины, и колонка схлопывается вместе с ней.
        .min_w(px(60.))
        .rounded(px(4.))
        .bg(native_color(style, SystemColor::Field))
        .border_1()
        .border_color(native_color(style, SystemColor::ButtonBorder))
        .child(
            div()
                .h_full()
                .w(gpui::relative(frac))
                .rounded(px(4.))
                .bg(accent),
        )
        .into_any_element()
}
