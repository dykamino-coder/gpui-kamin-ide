//! Paint decor for decor; split out to keep the owning module within 250 lines.

mod items;

use super::DecorLine;
use super::*;
use gpui::{Pixels, Window};

impl Paragraph {
    /// Lines of the decorations over `frag` (bytes of one painted fragment;
    /// `x_of` maps a byte to its x, `baseline` is the fragment's baseline in
    /// logical px before device snapping).
    pub(crate) fn paint_decor(
        &self,
        frag: std::ops::Range<usize>,
        x_of: &dyn Fn(usize, usize) -> (Pixels, Pixels),
        baseline: Pixels,
        dy: Pixels,
        line: &std::ops::Range<usize>,
        through: bool,
        window: &mut Window,
    ) {
        let scale = window.scale_factor();
        let mut segs: Option<Vec<Seg>> = None;
        for span in &self.decor_spans {
            let (s, e) = (
                span.range.start.max(frag.start),
                span.range.end.min(frag.end),
            );
            if s >= e {
                continue;
            }
            for (s, e) in self.skip_parts(span.skip_spaces, s, e, line) {
                let (xa, xb) = self.skip_tracking(span.skip_spaces, e, line, x_of(s, e));
                let (xa, xb) = (f32::from(xa) * scale, f32::from(xb) * scale);
                let (fx0, fx1) = (xa.min(xb), xa.max(xb));
                if fx1 <= fx0 {
                    continue;
                }
                self.paint_decor_items(
                    span, through, s, e, line, fx0, fx1, scale, baseline, dy, &mut segs, window,
                    x_of,
                );
            }
        }
    }
}
