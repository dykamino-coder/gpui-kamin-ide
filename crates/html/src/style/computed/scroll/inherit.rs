//! Inherited scrollbar colors and computed overflow-axis normalization.

use super::{Color, Computed};

pub(crate) fn inherit(parent: &Computed, c: &mut Computed) {
    normalize_overflow(c);
    super::super::snap::inherit(parent, c);
    // Only color inherits by default (CSS Scrollbars 1 §2). Width and gutter
    // remain local to the scroll container, including a nested one.
    if let Some(Some(mut colors)) = c.scroll.colors {
        let current = c.color.or(parent.color).unwrap_or(Color {
            a: 1.0,
            ..Color::default()
        });
        for (i, color) in colors.iter_mut().enumerate() {
            if c.scroll.current[i] {
                *color = current;
            }
        }
        c.scroll.colors = Some(Some(colors));
    }
    c.scroll.colors = c.scroll.colors.or(parent.scroll.colors);
    if c.scroll.inherit_gutter {
        c.scroll.gutter = parent.scroll.gutter;
    }
    if c.scroll.inherit_width {
        c.scroll.width = parent.scroll.width;
    }
}

/// CSS Overflow 3 §3: visible/clip compute to auto/hidden when the other axis
/// is neither visible nor clip. An absent declaration is initial `visible`.
fn normalize_overflow(c: &mut Computed) {
    use crate::style::computed::Overflow;
    let non_scroll = |o| matches!(o, None | Some(Overflow::Visible | Overflow::Clip));
    if non_scroll(c.overflow_x) && non_scroll(c.overflow_y) {
        return;
    }
    for axis in [&mut c.overflow_x, &mut c.overflow_y] {
        *axis = match *axis {
            None | Some(Overflow::Visible) => Some(Overflow::Scroll),
            Some(Overflow::Clip) => Some(Overflow::Hidden),
            other => other,
        };
    }
}
