//! Нужна ли боксу intrinsic-обёртка: предпочтительный размер `min-content`,
//! `max-content` или `fit-content`, который должен видеть родитель (css-sizing-3
//! §5), кроме случаев, когда size containment или вынос из потока делает размер
//! содержимого несущественным.

use crate::dom::Element;
use crate::layout::writing_mode::native_intrinsic;
use crate::style::values::value::Len;

pub(crate) fn content_sized_wraps(element: &Element) -> bool {
    if native_intrinsic::eligible(element) {
        return false;
    }
    let c = &element.style.used();
    let keyword = |l: Option<Len>| {
        matches!(
            l,
            Some(Len::MinContent) | Some(Len::MaxContent) | Some(Len::FitContent)
        )
    };
    // Обособленная ось содержимого не видит: размер по нему заменяется
    // `contain-intrinsic-size` (css-contain-2 §size-containment), и мерить
    // дорожкой сетки больше нечего.
    let contained = (keyword(c.reset_box_layout.width.clone()) && c.contains_width())
        || (keyword(c.reset_box_layout.height.clone()) && c.contains_height());
    if contained {
        return false;
    }
    (keyword(c.reset_box_layout.width.clone()) || keyword(c.reset_box_layout.height.clone()))
        && !matches!(
            c.reset_positioned.position,
            Some(crate::style::computed::Position::Absolute)
                | Some(crate::style::computed::Position::Fixed)
        )
}
