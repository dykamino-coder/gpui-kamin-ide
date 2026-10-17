//! Элементы выбора формы: select, checkbox и radio.

use super::{SystemColor, field_box, native_color};
use crate::dom::Element;
use crate::style::apply::apply;
use crate::style::computed::Computed;
use gpui::{AnyElement, IntoElement, ParentElement, SharedString, Styled, div, px};

/// Список выбора: показываем выбранный вариант либо первый.
pub(super) fn select(e: &Element, style: &Computed) -> AnyElement {
    let options = super::options::Options::from_element(e);
    let text = options.selected_label().to_owned();
    let label = if text.is_empty() {
        "\u{a0}".to_owned()
    } else {
        text
    };
    // «Sizing as if empty»: под обособлением СТРОЧНОЙ оси коробка мерится
    // так, будто содержимого нет вовсе — «not even through pseudo-elements»
    // (css-contain-2 Overview.bs:627-630). Подпись выбранного пункта уходит в
    // наложенный слой: мериться перестаёт, рисоваться продолжает — это второй
    // такт, «laying out in-place» (там же, :702-707). Без этого
    // `<select style="width:100px; contain:size">` растягивался по самому
    // длинному `option`.
    if style.contains_width() {
        return field_box(style)
            .relative()
            .justify_between()
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .overflow_hidden()
                    .child(SharedString::from(label.clone())),
            )
            .child(
                div()
                    .text_color(native_color(style, SystemColor::GrayText))
                    .child(SharedString::from("⌄")),
            )
            .into_any_element();
    }
    // `appearance: none` drops the drop-down arrow, the native part of the
    // menulist (css-ui-4 §7.2); the label stays as the box's content.
    if style.appearance_none() {
        return field_box(style)
            .child(SharedString::from(label.clone()))
            .into_any_element();
    }
    let mut b = field_box(style);
    // Пол `min_h(24)` у поля — для ПУСТОГО списка. С подписью он лишь
    // подменял автоминимум элемента гибкого контейнера (css-flexbox-1 §4.5:
    // `min-height: auto` = высота содержимого) явным 24, и в колонке высоты 0
    // список сжимался ниже своей строки (`select-element-zero-height-001/002`).
    if style.min_height.is_none() {
        b.style().min_size.height = None;
    }
    // All options contribute to the menu's max-content width; only the
    // selected label paints. A zero-height sizing stack uses the regular
    // text shaper (including proportional fonts), with no per-frame probe.
    let content = div()
        .min_w_0()
        .overflow_hidden()
        .child(SharedString::from(label))
        .child(
            div().h(px(0.)).overflow_hidden().opacity(0.).children(
                options
                    .labels
                    .into_iter()
                    .map(|label| div().whitespace_nowrap().child(SharedString::from(label))),
            ),
        );
    b.justify_between()
        .child(content)
        // Стрелка рисуется символом: своей иконки у документа нет, а без неё
        // список неотличим от обычного поля.
        .child(
            div()
                .flex_shrink_0()
                .text_color(native_color(style, SystemColor::GrayText))
                .child(SharedString::from("⌄")),
        )
        .into_any_element()
}

/// Флажок и переключатель отличаются только скруглением и отметкой.
pub(super) fn toggle(e: &Element, style: &Computed, round: bool) -> AnyElement {
    let on = e.attr("checked").is_some();
    // `accent-color` красит именно отметку флажка и переключателя — это
    // единственное, на что оно влияет.
    let accent: gpui::Hsla = super::control_accent(style)
        .map(|c| c.to_hsla())
        .unwrap_or_else(|| native_color(style, SystemColor::AccentColor));
    let natural = crate::style::ua::forms::size(if round { "radio" } else { "checkbox" });
    let mut mark = div()
        .absolute()
        .inset_0()
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(if on {
            accent
        } else {
            native_color(style, SystemColor::ButtonBorder)
        })
        .bg(if on {
            accent
        } else {
            native_color(style, SystemColor::Field)
        });
    mark = if round {
        mark.rounded_full()
    } else {
        mark.rounded(px(3.))
    };
    if on {
        mark = mark.child(
            div()
                .text_size(px(10.))
                .text_color(native_color(style, SystemColor::AccentColorText))
                .child(SharedString::from(if round { "•" } else { "✓" })),
        );
    }
    // Native paint does not add CSS layout borders: a zero-sized control
    // remains empty, and authored dimensions replace only the UA defaults.
    let paint = apply(div().relative(), style).overflow_hidden().child(mark);
    super::intrinsic::control(paint, natural)
}
