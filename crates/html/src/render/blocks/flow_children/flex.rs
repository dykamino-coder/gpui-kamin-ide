//! Используемые размеры и растяжение элемента гибкого контейнера.

use crate::dom::Node;

use crate::style::computed::{Align, Computed, FlexDir};
use crate::style::values::value::Len;

#[allow(clippy::too_many_arguments)]
pub(crate) fn flex_child(mut e: crate::dom::Element, inherited: &Computed) -> Node {
    if e.style.flex_shrink.is_none() {
        e.style.flex_shrink = Some(1.0);
    }
    // `vertical-align` на элементе гибкого контейнера не
    // действует (css-flexbox-1 §4): он выравнивается своими
    // свойствами, а не как кусок строки.
    e.style.vertical_shift = None;
    e.style.vertical_shift_px = None;
    // Элемент ряда под обособлением строчной оси: главный
    // размер берётся из `contain-intrinsic-size`, а не от
    // содержимого. В колонке главная ось блочная — её уже
    // держит подмена высоты.
    let row = !matches!(
        inherited.flex_dir,
        Some(FlexDir::Col) | Some(FlexDir::ColReverse)
    );
    if row && e.style.contains_width() && matches!(e.style.width, None | Some(Len::Auto)) {
        e.style.width = Some(Len::Px(e.style.contain_intrinsic.0.unwrap_or(0.0)));
    }
    // Поперечный размер элемента ряда с `height: auto` при
    // растяжке даёт строка (css-flexbox-1 §9.4 п.11), и
    // обособление высоты обязано ей уступить (`apply_box`).
    // `auto`-поле по поперечной оси растяжку отменяет.
    let stretches = e.style.align_self_normal
        || match e.style.align_self {
            Some(Align::Stretch) => true,
            None => matches!(inherited.align_items, None | Some(Align::Stretch)),
            _ => false,
        };
    if row
        && inherited.vertical != Some(true)
        && stretches
        && e.style.contains_height()
        && matches!(e.style.height, None | Some(Len::Auto))
        && e.style.margin.top != Some(Len::Auto)
        && e.style.margin.bottom != Some(Len::Auto)
    {
        e.style.cross_stretched = true;
    }
    Node::Element(e)
}
