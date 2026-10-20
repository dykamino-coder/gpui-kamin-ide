//! Whether a box needs an intrinsic-sizing wrapper: a `min-content`,
//! `max-content` or `fit-content` preferred size that its parent must see
//! (css-sizing-3 §5), unless size containment or out-of-flow positioning
//! makes the content size irrelevant.

use crate::dom::Element;
use crate::layout::writing_mode::native_intrinsic;
use crate::style::values::value::Len;

pub(crate) fn content_sized_wraps(element: &Element) -> bool {
    if native_intrinsic::eligible(element) {
        return false;
    }
    let c = &element.style;
    let keyword = |l: Option<Len>| {
        matches!(
            l,
            Some(Len::MinContent) | Some(Len::MaxContent) | Some(Len::FitContent)
        )
    };
    // Обособленная ось содержимого не видит: размер по нему заменяется
    // `contain-intrinsic-size` (css-contain-2 §size-containment), и мерить
    // дорожкой сетки больше нечего.
    let contained =
        (keyword(c.width) && c.contains_width()) || (keyword(c.height) && c.contains_height());
    if contained {
        return false;
    }
    (keyword(c.width) || keyword(c.height))
        && !matches!(
            c.position,
            Some(crate::style::computed::Position::Absolute)
                | Some(crate::style::computed::Position::Fixed)
        )
}
