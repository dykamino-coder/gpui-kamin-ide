//! Предикат для обёртки размера по содержимому: заполняет ли коробка ширину
//! родителя. Обёртка только по высоте не должна сжимать `width: auto` блока
//! в потоке — такая коробка растягивается на дорожку, а не прижимается к началу.
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;

pub(super) fn fills_width(style: &Computed) -> bool {
    !style.wm().vertical()
        && style.reset_context.inline_display != Some(true)
        && style.reset_box_layout.float.unwrap_or(0) == 0
        && matches!(style.reset_box_layout.width, None | Some(Len::Auto))
        && matches!(
            style.reset_box_layout.display,
            None | Some(Display::Block | Display::Flex | Display::Grid | Display::GridLanes)
        )
}
