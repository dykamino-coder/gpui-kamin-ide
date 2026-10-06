//! A height-only intrinsic wrapper must preserve automatic block width independently.
use crate::computed::{Computed, Display};
use crate::value::Len;

pub(super) fn fills_width(style: &Computed) -> bool {
    style.vertical != Some(true)
        && style.inline_display != Some(true)
        && style.float.unwrap_or(0) == 0
        && matches!(style.width, None | Some(Len::Auto))
        && matches!(
            style.display,
            None | Some(Display::Block | Display::Flex | Display::Grid | Display::GridLanes)
        )
}
