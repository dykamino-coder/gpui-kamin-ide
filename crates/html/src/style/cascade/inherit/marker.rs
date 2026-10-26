//! Стиль содержимого внешнего маркера, отдельный от главной коробки пункта.

use super::inherit;
use crate::document::state::ValueCx;
use crate::style::computed::{BoxStyle, Computed};
use crate::style::values::value::Len;

pub(crate) fn symbol(marker: Option<BoxStyle>, item: &BoxStyle, vcx: ValueCx<'_>) -> BoxStyle {
    let mut style = marker.unwrap_or_else(|| inherit(item, &Computed::default(), vcx));
    // Геометрический символ не имеет текстового tracking или межсловного зазора:
    // Blink ListMarker::InlineMarginsForOutside привязывает сам знак, а не пробел.
    style.letter_spacing = Some(Len::Px(0.0));
    style.word_spacing = Some(Len::Px(0.0));
    style
}

pub(crate) fn content(marker: Option<BoxStyle>, item: &BoxStyle, vcx: ValueCx<'_>) -> BoxStyle {
    let mut style = marker.unwrap_or_else(|| inherit(item, &Computed::default(), vcx));
    // Анонимный маркер наследует текст, но не размеры главного бокса.
    // Его строчная мера — содержимое, в том числе у вертикального абзаца.
    style.hug_inline = true;
    // Маркер — одна строка; это стиль созданной коробки псевдоэлемента.
    style.nowrap = Some(true);
    // Внешний маркер вне потока: фиксированная мера строки пункта
    // не должна растянуть его абзац перед привязкой к inline-start.
    style.orthogonal_inline = None;
    // css-lists-3 §marker-properties: text-indent не применяется к ::marker.
    // Абзац служит геометрией маркера, а не первой строкой главного бокса.
    style.text_indent = None;
    style.text_indent_each_line = None;
    style.text_indent_hanging = None;
    style
}
