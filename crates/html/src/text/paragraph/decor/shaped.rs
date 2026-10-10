//! Shaped for decor; split out to keep the owning module within 250 lines.

use super::*;
use gpui::{Pixels, Point, Window, px};

impl Paragraph {
    /// Lines are painted by the paragraph itself (not by GPUI's run
    /// underline) for horizontal text with decorated pieces.
    pub(crate) fn decor_on(&self) -> bool {
        !self.decor_spans.is_empty()
    }
}

impl Paragraph {
    /// Ascent (logical px) of the run that holds byte `at`.
    pub(crate) fn run_ascent_at(&self, at: usize) -> Option<f32> {
        if self.run_metrics.len() != self.runs.len() {
            return None;
        }
        let mut acc = 0usize;
        for (r, m) in self.runs.iter().zip(&self.run_metrics) {
            let st = acc;
            acc += r.len;
            if at >= st && at < acc {
                return Some(m.0);
            }
        }
        None
    }
}

impl Paragraph {
    /// Decorations of one painted shaped run (`run` — bytes of the paragraph,
    /// `at` — the run's line-box origin, as given to `ShapedLine::paint`).
    pub(crate) fn paint_decor_shaped(
        &self,
        run: &std::ops::Range<usize>,
        shaped: &gpui::ShapedLine,
        at: Point<Pixels>,
        dy: Pixels,
        rtl: bool,
        line: &std::ops::Range<usize>,
        through: bool,
        window: &mut Window,
    ) {
        let src = &self.text[run.clone()];
        if rtl {
            // Right-to-left run: the extent of `a..b` is the union of the
            // advances of its glyphs (visual order), whole run when covered.
            let lead = shaped.text.as_ref().find(src).unwrap_or(0);
            let start = run.start;
            let width = shaped.width;
            let mut boxes: Vec<(usize, Pixels, Pixels)> = Vec::new();
            let glyphs: Vec<&gpui::ShapedGlyph> =
                shaped.runs.iter().flat_map(|r| r.glyphs.iter()).collect();
            for (k, g) in glyphs.iter().enumerate() {
                let right = glyphs.get(k + 1).map_or(width, |n| n.position.x);
                boxes.push((g.index, g.position.x, right));
            }
            // `shape_line_rtl` puts clusters in visual order and renumbers
            // their indices visually: a cluster at visual `v..v_next` holds
            // the logical bytes `len - v_next..len - v` of the shaped text.
            let total = shaped.len;
            let mut clusters: Vec<(usize, Pixels, Pixels)> = Vec::new();
            for &(v, l, r) in &boxes {
                match clusters.last_mut() {
                    Some(c) if c.0 == v => {
                        c.1 = c.1.min(l);
                        c.2 = c.2.max(r);
                    }
                    _ => clusters.push((v, l, r)),
                }
            }
            let logical: Vec<(usize, usize, Pixels, Pixels)> = clusters
                .iter()
                .enumerate()
                .map(|(k, &(v, l, r))| {
                    let next = clusters.get(k + 1).map_or(total, |c| c.0);
                    (total.saturating_sub(next), total.saturating_sub(v), l, r)
                })
                .collect();
            let text = &self.text;
            let x_of = |a: usize, b: usize| -> (Pixels, Pixels) {
                if a <= start && b >= start + src.len() {
                    return (at.x, at.x + width);
                }
                let mut ext: Option<(Pixels, Pixels)> = None;
                let (a, b) = (a.max(start), b.min(start + src.len()));
                for (i, ch) in text[a..b].char_indices() {
                    // Joiners and other format characters draw nothing.
                    if matches!(ch, '\u{200b}'..='\u{200f}' | '\u{2060}'..='\u{206f}' | '\u{feff}' | '\u{202a}'..='\u{202e}')
                    {
                        continue;
                    }
                    let o = lead + a + i - start;
                    if let Some(&(_, _, l, r)) = logical.iter().find(|c| c.0 <= o && o < c.1) {
                        ext = Some(ext.map_or((l, r), |(el, er)| (el.min(l), er.max(r))));
                    }
                }
                let (l, r) = ext.unwrap_or((px(0.), px(0.)));
                (at.x + l, at.x + r)
            };
            let baseline =
                at.y + (self.line_height - shaped.ascent - shaped.descent) / 2.0 + shaped.ascent;
            self.paint_decor(run.clone(), &x_of, baseline, dy, line, through, window);
            return;
        }
        let lead = shaped.text.as_ref().find(src).unwrap_or(0);
        let baseline =
            at.y + (self.line_height - shaped.ascent - shaped.descent) / 2.0 + shaped.ascent;
        let start = run.start;
        let width = shaped.width;
        let x_one = |i: usize| -> Pixels {
            if i <= start {
                at.x + if lead == 0 {
                    px(0.)
                } else {
                    shaped.x_for_index(lead)
                }
            } else if i - start >= src.len() {
                // The run end: its advance, not the appended marker's.
                at.x + if lead + src.len() >= shaped.text.len() {
                    width
                } else {
                    shaped.x_for_index(lead + src.len())
                }
            } else {
                at.x + shaped.x_for_index(lead + i - start)
            }
        };
        let x_of = |a: usize, b: usize| (x_one(a), x_one(b));
        self.paint_decor(run.clone(), &x_of, baseline, dy, line, through, window);
    }
}

impl Paragraph {
    /// Vertical shift of the decorating box of `group`: the underline keeps
    /// to its decorating box, not to a shifted descendant
    /// (`text-decoration-decorating-box-001`). The box's own shift is the
    /// one most of the decorated run carries.
    pub(crate) fn group_shift(&self, group: &std::ops::Range<usize>) -> Pixels {
        let total = group.end.saturating_sub(group.start);
        let mut tally: Vec<(Pixels, usize)> = Vec::new();
        let mut shifted = 0usize;
        for (r, v) in &self.shift_spans {
            let n = r
                .end
                .min(group.end)
                .saturating_sub(r.start.max(group.start));
            if n == 0 {
                continue;
            }
            shifted += n;
            match tally.iter_mut().find(|(t, _)| t == v) {
                Some(slot) => slot.1 += n,
                None => tally.push((*v, n)),
            }
        }
        tally.push((px(0.), total.saturating_sub(shifted)));
        tally
            .iter()
            .max_by_key(|(_, n)| *n)
            .map_or(px(0.), |(v, _)| *v)
    }
}
