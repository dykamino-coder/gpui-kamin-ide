//! Ruby annotation overhang (css-ruby-1 §4.4 `ruby-overhang`).
//!
//! An annotation wider than its base may overhang the content next to the
//! ruby: the ruby's advance in the line shrinks by the overhang on each side,
//! and the atom is drawn shifted left by the start overhang. The amounts follow
//! Blink `ruby_utils.cc` (`GetOverhang`, `CommitPendingEndOverhang`):
//! - `auto`: by at most half the annotation font size on each side, and not
//!   more than the base inset (half the excess for centred annotations);
//!   the end overhang covers at most half of the following text and never
//!   text set in a larger font than the base.
//! - `spaces`: only over adjacent space separators (General Category Zs,
//!   which includes U+0020, U+00A0 and U+3000), up to the base inset.
//! - `none`: never.

use super::*;
use crate::style::computed::RubyOverhang;
use gpui::Window;

/// Space separators a ruby annotation may overhang (Blink
/// `IsSpaceForRubyOverhang`: Unicode General Category Zs).
fn is_space_separator(c: char) -> bool {
    matches!(
        c,
        ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}' | '\u{202f}' | '\u{205f}' | '\u{3000}'
    )
}

/// A character that belongs to text content (not an atom spacer or a line
/// break), i.e. something an annotation could overhang.
fn is_text_char(c: char) -> bool {
    !matches!(
        c,
        '\u{feff}' | '\u{fffc}' | '\n' | '\r' | '\u{2028}' | '\u{2029}'
    )
}

impl Paragraph {
    /// Font size of the text run covering byte `at`.
    fn run_font_at(&self, at: usize) -> f32 {
        let mut start = 0usize;
        for run in &self.runs {
            let end = start + run.len;
            if at < end {
                return f32::from(run.font_size.unwrap_or(self.font_size));
            }
            start = end;
        }
        f32::from(self.font_size)
    }

    /// Start and end overhang of the ruby atom whose spacer is
    /// `at..at + len`; `w` is the atom width, `base_w` the base content
    /// width and `ann_w` the widest annotation level.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn ruby_overhang(
        &self,
        info: &RubyOverhangInfo,
        at: usize,
        len: usize,
        w: f32,
        base_w: f32,
        ann_w: f32,
        window: &mut Window,
    ) -> (f32, f32) {
        if info.mode == RubyOverhang::None || ann_w <= base_w + 0.01 {
            return (0.0, 0.0);
        }
        let space = w - base_w;
        if space <= 0.0 {
            return (0.0, 0.0);
        }
        // Centred annotation (`center`, and `space-around` over a base without
        // inner expansion opportunities): the base sits `space / 2` in.
        let inset = space / 2.0;
        let half = info.half_annotation_font;
        let text = self.text.as_ref();
        let after = (at + len).min(text.len());
        let segs = self.measure(window);
        // Run of space separators right before / right after the spacer.
        let mut pre = at;
        for (i, c) in text[..at].char_indices().rev() {
            if !is_space_separator(c) {
                break;
            }
            pre = i;
        }
        let mut post = after;
        for (i, c) in text[after..].char_indices() {
            if !is_space_separator(c) {
                break;
            }
            post = after + i + c.len_utf8();
        }
        let prev_text = text[..at].chars().next_back().is_some_and(is_text_char);
        let next_text = text[after..].chars().next().is_some_and(is_text_char);
        match info.mode {
            RubyOverhang::None => (0.0, 0.0),
            RubyOverhang::Spaces => {
                let pre_w = f32::from(self.span(&segs, pre, at));
                let post_w = f32::from(self.span(&segs, after, post));
                if info.align_start {
                    return (0.0, (inset * 2.0).min(space).min(post_w));
                }
                (inset.min(pre_w), inset.min(post_w))
            }
            RubyOverhang::Auto => {
                // The end overhang covers at most half of the following text
                // item, and never text in a larger font than the base.
                let next_end = text[after..]
                    .char_indices()
                    .find(|&(_, c)| !is_text_char(c))
                    .map_or(text.len(), |(i, _)| after + i);
                let next_w = f32::from(self.span(&segs, after, next_end));
                let end_cap = if next_text && self.run_font_at(after) <= info.base_font + 0.01 {
                    next_w / 2.0
                } else {
                    0.0
                };
                if info.align_start {
                    return (0.0, space.min(half).min(end_cap));
                }
                let start = if prev_text { inset.min(half) } else { 0.0 };
                (start, inset.min(half).min(end_cap))
            }
        }
    }
}
