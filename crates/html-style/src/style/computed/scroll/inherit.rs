//! Наследуемые цвета полос прокрутки и нормализация вычисленных осей overflow.

use super::Computed;

pub fn inherit(parent: &Computed, c: &mut Computed) {
    normalize_overflow(c);
    super::super::snap::inherit(parent, c);
    // Only color inherits by default (CSS Scrollbars 1 §2). Width and gutter
    // remain local to the scroll container, including a nested one.
    if c.reset_box_layout.scroll.colors.is_none() {
        c.reset_box_layout.scroll.colors = parent.reset_box_layout.scroll.colors.clone();
    }
    if c.reset_box_layout.scroll.inherit_gutter {
        c.reset_box_layout.scroll.gutter = parent.reset_box_layout.scroll.gutter;
    }
    if c.reset_box_layout.scroll.inherit_width {
        c.reset_box_layout.scroll.width = parent.reset_box_layout.scroll.width;
    }
}

/// CSS Overflow 3 §3: visible/clip compute to auto/hidden when the other axis
/// is neither visible nor clip. An absent declaration is initial `visible`.
fn normalize_overflow(c: &mut Computed) {
    use crate::style::computed::Overflow;
    let non_scroll = |o| matches!(o, None | Some(Overflow::Visible | Overflow::Clip));
    if non_scroll(c.reset_box_layout.overflow_x) && non_scroll(c.reset_box_layout.overflow_y) {
        return;
    }
    for axis in {
        let group_0 = &mut *c.reset_box_layout;
        [&mut group_0.overflow_x, &mut group_0.overflow_y]
    } {
        *axis = match *axis {
            None | Some(Overflow::Visible) => Some(Overflow::Scroll),
            Some(Overflow::Clip) => Some(Overflow::Hidden),
            other => other,
        };
    }
}
