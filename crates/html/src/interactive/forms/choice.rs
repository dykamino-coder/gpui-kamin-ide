//! Элементы выбора формы: select, checkbox и radio.

use super::{ACCENT, BORDER, FIELD_BG, MUTED, field_box};
use crate::dom::Element;
use crate::style::apply::apply;
use crate::style::computed::Computed;
use gpui::{AnyElement, IntoElement, ParentElement, SharedString, Styled, div, px, rgb};

/// Список выбора: показываем выбранный вариант либо первый.
pub(super) fn select(e: &Element, style: &Computed) -> AnyElement {
    let mut chosen: Option<String> = None;
    let mut first: Option<String> = None;
    for child in &e.children {
        let crate::dom::Node::Element(opt) = child else {
            continue;
        };
        if opt.tag != "option" {
            continue;
        }
        let mut label = String::new();
        crate::render::gather_text_public(&opt.children, &mut label);
        let label = label.trim().to_string();
        if first.is_none() {
            first = Some(label.clone());
        }
        if opt.attr("selected").is_some() {
            chosen = Some(label);
        }
    }
    let text = chosen.or(first).unwrap_or_default();
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
                    .child(SharedString::from(text)),
            )
            .child(div().text_color(rgb(MUTED)).child(SharedString::from("⌄")))
            .into_any_element();
    }
    let mut b = field_box(style);
    // Пол `min_h(24)` у поля — для ПУСТОГО списка. С подписью он лишь
    // подменял автоминимум элемента гибкого контейнера (css-flexbox-1 §4.5:
    // `min-height: auto` = высота содержимого) явным 24, и в колонке высоты 0
    // список сжимался ниже своей строки (`select-element-zero-height-001/002`).
    if !text.is_empty() && style.min_height.is_none() {
        b.style().min_size.height = None;
    }
    b.justify_between()
        .child(SharedString::from(text))
        // Стрелка рисуется символом: своей иконки у документа нет, а без неё
        // список неотличим от обычного поля.
        .child(div().text_color(rgb(MUTED)).child(SharedString::from("⌄")))
        .into_any_element()
}

/// Флажок и переключатель отличаются только скруглением и отметкой.
pub(super) fn toggle(e: &Element, style: &Computed, round: bool) -> AnyElement {
    let on = e.attr("checked").is_some();
    // `accent-color` красит именно отметку флажка и переключателя — это
    // единственное, на что оно влияет.
    let accent: gpui::Hsla = super::control_accent(style)
        .map(|c| c.to_hsla())
        .unwrap_or_else(|| rgb(ACCENT).into());
    let mut mark = apply(div(), style)
        .w(px(15.))
        .h(px(15.))
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(if on { accent } else { rgb(BORDER).into() })
        .bg(if on { accent } else { rgb(FIELD_BG).into() });
    mark = if round {
        mark.rounded_full()
    } else {
        mark.rounded(px(3.))
    };
    if on {
        mark = mark.child(
            div()
                .text_size(px(10.))
                .text_color(rgb(0xffffff))
                .child(SharedString::from(if round { "•" } else { "✓" })),
        );
    }
    mark.into_any_element()
}
