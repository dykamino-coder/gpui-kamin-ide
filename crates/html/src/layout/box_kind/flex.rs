//! Классификация направления и переноса flex-контейнера без физических осей.

use crate::style::computed::{Computed, Display, FlexDir};

/// Ряд flex: начальное направление — row, обратный ряд остаётся рядом.
pub(crate) fn flex_row(style: &Computed) -> bool {
    matches!(style.display, Some(Display::Flex | Display::InlineFlex))
        && matches!(
            style.flex_dir,
            None | Some(FlexDir::Row | FlexDir::RowReverse)
        )
}

/// Однострочный ряд; начальный flex-wrap — nowrap (css-flexbox-1 §5.2).
pub(crate) fn flex_row_nowrap(style: &Computed) -> bool {
    flex_row(style) && style.flex_wrap != Some(true)
}
