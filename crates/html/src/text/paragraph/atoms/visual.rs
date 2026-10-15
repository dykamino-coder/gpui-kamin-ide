//! Atomic inlines use the same UAX #9 visual runs as the painted text.

use crate::text::paragraph::*;
use gpui::{Pixels, px};

impl Paragraph {
    pub(crate) fn atom_inline_x(&self, segs: &[Seg], line: &Line, at: usize) -> Pixels {
        let len = self.text[at..].chars().next().map_or(0, char::len_utf8);
        let advance = self.span(segs, at, at + len);
        let runs = self.line_visual_runs(&line.range);
        if runs.is_empty() {
            let logical = self.span(segs, line.range.start, at);
            return if self.wrap.rtl {
                line.width - logical - advance
            } else {
                logical
            };
        }
        let mut x = px(0.0);
        for (start, end, rtl) in runs {
            if (start..end).contains(&at) {
                return x + if rtl {
                    self.span(segs, at + len, end)
                } else {
                    self.span(segs, start, at)
                };
            }
            x += self.span(segs, start, end);
        }
        self.span(segs, line.range.start, at)
    }
}
