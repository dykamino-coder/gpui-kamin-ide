//! Перевод стиля коробки в элемент GPUI один раз на равный вход сборки: коробки с равными
//! значениями, размерами и контекстом значений получают один общий `StyleRefinement`
//! (Blink: `MatchedPropertiesCache`; Stylo: общий `ComputedValues` у братьев).

use super::{apply_for_decorations_with_dimensions, child_paint_policy};
use crate::layout::block::sizing::BoxDimensions;
use crate::layout::cx::conversions::{Lookup, StyleConversions};
use crate::style::computed::BoxStyle;
use crate::values::ValueCx;
use gpui::{Div, InteractiveElement, div};

/// `apply_for_decorations_with_dimensions(div(), c, Some(dimensions), vcx)` through the memory
/// of converted styles of the current document build (none outside builds). The conversion
/// writes only the element's style and its child paint policy; a hit takes the shared style
/// (no allocation: a new div starts from the shared empty style) and recomputes the policy.
pub(crate) fn converted_div(
    c: &BoxStyle,
    dimensions: BoxDimensions,
    vcx: ValueCx<'_>,
    memo: Option<&StyleConversions>,
) -> Div {
    let Some(memo) = memo else {
        return apply_for_decorations_with_dimensions(div(), c, Some(dimensions), vcx);
    };
    let lookup = memo.lookup(c, &dimensions, vcx);
    let mut d = div();
    match lookup {
        Lookup::Hit(style) => {
            #[cfg(test)]
            verify(&style, c, dimensions, vcx);
            d.interactivity().base_style = style;
            child_paint_policy(d, c)
        }
        Lookup::Miss => {
            if let Some(blank) = memo.blank() {
                d.interactivity().base_style = blank;
            }
            let mut d = apply_for_decorations_with_dimensions(d, c, Some(dimensions), vcx);
            let style = d.interactivity().base_style.clone();
            memo.record(c, dimensions, vcx, style);
            d
        }
        Lookup::Pass => {
            if let Some(blank) = memo.blank() {
                d.interactivity().base_style = blank;
            }
            apply_for_decorations_with_dimensions(d, c, Some(dimensions), vcx)
        }
    }
}

/// A reused style equals a fresh conversion of the same box.
#[cfg(test)]
fn verify(
    style: &gpui::StyleRefinement,
    c: &BoxStyle,
    dimensions: BoxDimensions,
    vcx: ValueCx<'_>,
) {
    let mut fresh = apply_for_decorations_with_dimensions(div(), c, Some(dimensions), vcx);
    assert_eq!(*fresh.interactivity().base_style, *style);
}

#[cfg(test)]
#[path = "conversion_tests.rs"]
mod tests;
