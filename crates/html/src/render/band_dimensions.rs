//! Resolve definite content widths before sharing nested float exclusions.

use crate::computed::Computed;
use crate::value::Len;

/// CSS 2.1 §§4.3.2, 9.5: an em width is definite once the element's font
/// size is known. It does not establish a new block formatting context;
/// its descendants must still participate in the containing BFC's floats.
pub(crate) fn content_width(style: &Computed, inherited_em: f32) -> Option<f32> {
    match style.width {
        Some(Len::Px(width)) => Some(width),
        Some(Len::Em(factor)) => Some(factor * super::band_em(style, inherited_em)?),
        _ => None,
    }
}
