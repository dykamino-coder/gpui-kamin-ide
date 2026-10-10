//! Ink for decor; split out to keep the owning module within 250 lines.

use super::is_cjk;
use super::*;
use gpui::{Pixels, TextRun, Window, px};

impl Paragraph {
    /// Inline size of the decorated text `a..b` over all lines (logical px):
    /// the fragments of a line end before its hanging or removed spaces.
    pub(crate) fn decor_width(&self, segs: &[Seg], a: usize, b: usize) -> f32 {
        const NL: char = 10 as char;
        const TAB: char = 9 as char;
        let mut w = 0.0;
        for line in &self.lines {
            let body = &self.text[line.range.clone()];
            let end = if self.wrap.keep_spaces {
                line.range.start + body.trim_end_matches(NL).len()
            } else {
                line.range.start + body.trim_end_matches([' ', TAB, NL]).len()
            };
            let (s, e) = (a.max(line.range.start), b.min(end));
            if s < e {
                w += f32::from(self.span(segs, s, e));
            }
        }
        w
    }
}

impl Paragraph {
    /// Paint the patterned lines merged so far (end of a painted line).
    pub(crate) fn flush_decor(&self, window: &mut Window) {
        let scale = window.scale_factor();
        let pending = std::mem::take(&mut *self.decor_pending.borrow_mut());
        for line in pending {
            line.paint(scale, window);
        }
    }
}

impl Paragraph {
    /// Ink boxes of the characters `s..e` in device px: (left, right, top,
    /// bottom), the vertical ones from the baseline (y down). Glyph boxes
    /// stand in for Blink's outline intercepts (`GetTextIntercepts`); CJK is
    /// excluded as in `kExcludeCJK`.
    pub(crate) fn ink_boxes(
        &self,
        s: usize,
        e: usize,
        x_of: &dyn Fn(usize, usize) -> (Pixels, Pixels),
        window: &mut Window,
    ) -> Vec<(f32, f32, f32, f32)> {
        let scale = window.scale_factor();
        let ts = window.text_system().clone();
        let mut out = Vec::new();
        let mut bounds: Vec<(std::ops::Range<usize>, &TextRun)> = Vec::new();
        let mut acc = 0usize;
        for r in &self.runs {
            bounds.push((acc..acc + r.len, r));
            acc += r.len;
        }
        for (i, ch) in self.text[s..e].char_indices() {
            let at = s + i;
            let Some((_, run)) = bounds.iter().find(|(r, _)| r.contains(&at)) else {
                continue;
            };
            if ch.is_whitespace() || is_cjk(ch) {
                continue;
            }
            let id = ts.resolve_font(&run.font);
            let size = run.font_size.unwrap_or(self.font_size);
            let Ok(b) = ts.typographic_bounds(id, size, ch) else {
                continue;
            };
            if b.size.width <= px(0.) || b.size.height <= px(0.) {
                continue;
            }
            let (xa, xb) = x_of(at, at + ch.len_utf8());
            let x = f32::from(xa.min(xb)) * scale;
            let left = x + f32::from(b.origin.x) * scale;
            let right = left + f32::from(b.size.width) * scale;
            // `origin.y` is the ink bottom with y up.
            let bottom = -f32::from(b.origin.y) * scale;
            let top = bottom - f32::from(b.size.height) * scale;
            out.push((left, right, top, bottom));
        }
        out
    }
}

impl Paragraph {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn decor_insets(
        &self,
        item: &DecorItem,
        s: usize,
        e: usize,
        line: &std::ops::Range<usize>,
        fx0: f32,
        fx1: f32,
        scale: f32,
        t: f32,
        segs: &mut Option<Vec<Seg>>,
        window: &mut Window,
    ) -> (f32, f32) {
        let d = &item.decor;
        // Ends: `text-decoration-inset` (css-text-decor-4 §4.1).
        let (mut l, mut r) = (0.0f32, 0.0f32);
        match d.inset {
            None => {
                let a = (t / 2.0).clamp(1.0, 2.0);
                (l, r) = (a, a);
            }
            Some([st, en]) if !(inset_zero(st) && inset_zero(en)) => {
                let w = fx1 - fx0;
                if d.clone {
                    let prev = s > item.group.start && s > line.start;
                    let line_end = line.start
                        + self.text[line.clone()]
                            .trim_end_matches([' ', 9u8 as char, 10u8 as char])
                            .len();
                    let next = e < item.group.end && e < line_end;
                    if !prev {
                        l = inset_px(st, w, scale);
                    }
                    if !next {
                        r = inset_px(en, w, scale);
                    }
                } else {
                    let segs = segs.get_or_insert_with(|| self.measure(window));
                    let before = self.decor_width(segs, item.group.start, s) * scale;
                    let after = self.decor_width(segs, e, item.group.end) * scale;
                    let total = before + w + after;
                    l = inset_for_fragment(inset_px(st, total, scale), before);
                    r = inset_for_fragment(inset_px(en, total, scale), after);
                }
            }
            _ => {}
        }
        if self.wrap.rtl {
            std::mem::swap(&mut l, &mut r);
        }
        (l, r)
    }
}
