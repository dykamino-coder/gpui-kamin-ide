//! Spacer exclusion and adjacent tracking for decoration intervals.

use super::*;
use unicode_properties::{GeneralCategory, UnicodeGeneralCategory};
use gpui::{Pixels, px};

fn spacer(c: char) -> bool {
    c != '\u{202f}' && c.general_category() == GeneralCategory::SpaceSeparator
}

impl Paragraph {
    /// `text-decoration-skip-spaces` (css-text-decor-4 §4.2): the parts of
    /// `s..e` that are decorated. Spacers are Zs characters except U+202F;
    /// `start`/`end` skip them at the start/end of the line, `all`
    /// everywhere (word separators too).
    pub(super) fn skip_parts(
        &self,
        skip: u8,
        s: usize,
        e: usize,
        line: &std::ops::Range<usize>,
    ) -> Vec<(usize, usize)> {
        if skip & 4 != 0 {
            let mut out = Vec::new();
            let mut cur: Option<usize> = None;
            for (i, c) in self.text[s..e].char_indices() {
                if spacer(c) {
                    if let Some(st) = cur.take() {
                        out.push((st, s + i));
                    }
                } else if cur.is_none() {
                    cur = Some(s + i);
                }
            }
            if let Some(st) = cur {
                out.push((st, e));
            }
            return out;
        }
        let (mut a, mut b) = (s, e);
        if skip & 1 != 0 {
            // Spacers from the line start up to `a` must all be spacers.
            let head = &self.text[line.start.min(a)..a];
            if head.chars().all(spacer) {
                let lead: usize = self.text[a..b]
                    .chars()
                    .take_while(|c| spacer(*c))
                    .map(char::len_utf8)
                    .sum();
                a += lead;
            }
        }
        if skip & 2 != 0 && a < b {
            let tail = &self.text[b..line.end.max(b)];
            if tail.chars().all(|c| spacer(c) || c == '\n') {
                let trail: usize = self.text[a..b]
                    .chars()
                    .rev()
                    .take_while(|c| spacer(*c))
                    .map(char::len_utf8)
                    .sum();
                b -= trail;
            }
        }
        if a < b { vec![(a, b)] } else { Vec::new() }
    }

    /// CSS Text Decoration 4 §4.2: `all` excludes the trailing tracking
    /// at the line end and next to a skipped spacer. A style boundary alone
    /// does not remove tracking between two decorated letters.
    pub(super) fn skip_tracking(
        &self,
        skip: u8,
        end: usize,
        line: &std::ops::Range<usize>,
        (a, b): (Pixels, Pixels),
    ) -> (Pixels, Pixels) {
        if skip & 4 == 0 {
            return (a, b);
        }
        let line_end = line.start + self.text[line.clone()].trim_end_matches('\n').len();
        if end < line_end && !self.text[end..].chars().next().is_some_and(spacer) {
            return (a, b);
        }
        let tracking = self.tail_spacing(end).max(px(0.));
        let at = self.text[..end]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i);
        if self.rtl_level_at(at) {
            (a + tracking, b)
        } else {
            (a, b - tracking)
        }
    }
}
