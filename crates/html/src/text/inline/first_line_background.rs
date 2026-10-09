//! Keep inline box backgrounds above the first-line pseudo-element's background.

use crate::style::values::value::Color;

pub(super) fn paint_color(child: Option<Color>, first_line: Option<Color>) -> Option<Color> {
    // CSS Pseudo 4 section 2.1.3: non-inherited properties inherit from the
    // non-pseudo parent. The first-line background is a lower paint layer,
    // rather than an override of backgrounds on its inline descendants.
    match (child, first_line) {
        (Some(child), Some(parent)) => Some(crate::paint::background::over(child, parent)),
        (child, None) => child,
        (None, parent) => parent,
    }
}
