//! Стиль содержимого внешнего маркера, отдельный от главной коробки пункта.

use super::inherit;
use crate::document::state::ValueCx;
use crate::style::computed::{BoxStyle, Computed};

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
    style
}
