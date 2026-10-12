//! CSS Ruby 1 §ruby-overhang: preserved whitespace and blank punctuation edges.
//! Blink ruby_utils.cc handles control advances and Han-kerning edges separately.

use super::is_space_separator;
use crate::text::paragraph::{Paragraph, Seg, slice_runs};
use gpui::{SharedString, Window, px};
use unicode_properties::{GeneralCategory, UnicodeGeneralCategory};

#[derive(Clone, Copy)]
enum Blank {
    Start,
    End,
    Both,
    Ambiguous,
}

// CSS Text 4 §text-spacing-trim: Unicode punctuation classes, with quotes
// added explicitly. Fullwidth colon/dot classification depends on the font.
fn punctuation(c: char) -> Option<Blank> {
    match c {
        '\u{2018}' | '\u{201c}' => Some(Blank::Start),
        '\u{2019}' | '\u{201d}' => Some(Blank::End),
        '\u{00b7}' | '\u{2027}' | '\u{30fb}' => Some(Blank::Both),
        '\u{3001}' | '\u{3002}' | '\u{ff0c}' | '\u{ff0e}' | '\u{ff1a}' | '\u{ff1b}' => {
            Some(Blank::Ambiguous)
        }
        _ if matches!(c, '\u{3000}'..='\u{303f}' | '\u{ff01}'..='\u{ff60}') => {
            match c.general_category() {
                GeneralCategory::OpenPunctuation => Some(Blank::Start),
                GeneralCategory::ClosePunctuation => Some(Blank::End),
                _ => None,
            }
        }
        _ => None,
    }
}

impl Paragraph {
    pub(super) fn ruby_space_edge_width(
        &self,
        segs: &[Seg],
        edge: usize,
        before: bool,
        window: &mut Window,
    ) -> f32 {
        let mut boundary = edge;
        // Only preserved tabs survive inline whitespace processing. Their
        // measured advance is the distance to the actual tab stop, not 1em.
        let space = |c| is_space_separator(c) || c == '\t';
        let adjacent = if before {
            for (i, c) in self.text[..edge].char_indices().rev() {
                if !space(c) {
                    break;
                }
                boundary = i;
            }
            self.text[..boundary].char_indices().next_back()
        } else {
            for (i, c) in self.text[edge..].char_indices() {
                if !space(c) {
                    break;
                }
                boundary = edge + i + c.len_utf8();
            }
            self.text[boundary..].chars().next().map(|c| (boundary, c))
        };
        let blanks = f32::from(self.span(segs, edge.min(boundary), edge.max(boundary)));
        let Some((at, c)) = adjacent else {
            return blanks;
        };
        let Some(mut kind) = punctuation(c) else {
            return blanks;
        };
        let end = at + c.len_utf8();
        let advance = f32::from(self.span(segs, at, end) - self.tail_spacing(end));
        let size = self.run_font_at(at);
        // Blink IsFullWidthGlyph: reject halfwidth and already-trimmed forms.
        if advance <= size * 0.9 {
            return blanks;
        }
        if matches!(kind, Blank::Ambiguous) {
            let runs = slice_runs(&self.runs, &(at..end));
            let shaped = window.text_system().shape_line(
                SharedString::from(c.to_string()),
                px(size),
                &runs,
                None,
            );
            let bounds = shaped.runs.first().and_then(|run| {
                window
                    .text_system()
                    .typographic_bounds(run.font_id, px(size), c)
                    .ok()
            });
            let Some(bounds) = bounds else { return blanks };
            // Ink wholly on the inline-start half leaves the end half blank;
            // otherwise this colon/dot is centered (CSS Text 4 §fullwidth-ambiguous).
            kind = if f32::from(bounds.origin.x + bounds.size.width) <= advance / 2.0 {
                Blank::End
            } else {
                Blank::Both
            };
        }
        let fraction = match kind {
            Blank::Start if !before => 0.5,
            Blank::End if before => 0.5,
            Blank::Both => 0.25,
            _ => 0.0,
        };
        blanks + advance * fraction
    }
}
