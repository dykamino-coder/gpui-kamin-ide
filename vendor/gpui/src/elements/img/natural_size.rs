//! Keep an automatic image axis unresolved when its opposite axis is a percentage.
//! The layout algorithm resolves the percentage before transferring the intrinsic ratio.

use crate::{DefiniteLength, Length, Pixels, Size, Style, px};

pub(super) fn resolve(
    style: &mut Style,
    natural: Size<Pixels>,
    rem: Pixels,
) -> Option<Size<Pixels>> {
    let percentage =
        |length: Length| matches!(length, Length::Definite(DefiniteLength::Fraction(_)));
    if (matches!(style.size.width, Length::Auto) && percentage(style.size.height))
        || (matches!(style.size.height, Length::Auto) && percentage(style.size.width))
    {
        return Some(natural);
    }
    if matches!(style.size.width, Length::Auto) {
        style.size.width = match style.size.height {
            Length::Definite(DefiniteLength::Absolute(length)) => {
                px(natural.width.0 * length.to_pixels(rem).0 / natural.height.0).into()
            }
            _ => natural.width.into(),
        };
    }
    if matches!(style.size.height, Length::Auto) {
        style.size.height = match style.size.width {
            Length::Definite(DefiniteLength::Absolute(length)) => {
                px(natural.height.0 * length.to_pixels(rem).0 / natural.width.0).into()
            }
            _ => natural.height.into(),
        };
    }
    None
}
