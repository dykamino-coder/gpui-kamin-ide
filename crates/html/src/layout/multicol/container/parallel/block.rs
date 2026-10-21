//! Пригодность блочных смещений для независимой фрагментации позиционированных потоков.

use crate::dom::Element;
use crate::layout::multicol::spanner::multicol_container;
use crate::style::computed::Display;
use crate::style::values::value::Len;

pub(super) fn zero(v: Option<Len>) -> bool {
    matches!(v, None | Some(Len::Px(0.0)))
}

// Other formatting contexts own their block offsets. They need a positioning
// adapter before their descendants can participate in this block-flow pass.
pub(super) fn block(c: &Element) -> bool {
    let border = c.style.borders();
    !c.inline
        && matches!(c.style.display, None | Some(Display::Block))
        && !multicol_container(&c.style)
        && !c.style.bdb_clone
        && c.style.float.unwrap_or(0) == 0
        && zero(c.style.padding.top)
        && zero(c.style.padding.bottom)
        && zero(border.top)
        && zero(border.bottom)
}
