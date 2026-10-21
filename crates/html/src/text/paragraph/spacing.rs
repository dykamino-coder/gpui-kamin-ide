//! Трекинг и межсловный интервал силами абзаца: совместимый горизонтальный
//! текст набирается одним вызовом без скалярного трекинга GPUI, а добавки
//! накладываются на готовый набор по типографским единицам (графемам), а
//! не по глифам и управляющим знакам (css-text-3 §8.2; Blink
//! `line_breaker.cc` `ShapeText`) — смена трекинга на границе куска не
//! рвёт лигатуру. Здесь же условие этого пути (`joined_spacing`).

use super::*;
use gpui::{LineLayout, Pixels, ShapedLine, px};
use std::{ops::Range, sync::Arc};
use unicode_properties::{GeneralCategory, UnicodeGeneralCategory};
use unicode_segmentation::UnicodeSegmentation;

impl Paragraph {
    /// The plain horizontal path can apply all advances to the shaped line.
    /// Positioned pieces, tabs and bidi still use their visual placement path.
    pub(crate) fn joined_spacing(&self) -> bool {
        (self.letter_spacing != px(0.)
            || self.word_spacing != px(0.)
            || self
                .letter_spans
                .iter()
                .chain(&self.word_spans)
                .any(|(_, v)| *v != px(0.)))
            && !self.vertical
            && !self.wrap.rtl
            // Atom carriers use a formatting character's tracking as their
            // physical advance, not as CSS letter spacing (CSS 2.1 §9.2.2).
            && self.atom_boxes.is_empty()
            && self.atoms.is_empty()
            && self.spacers.is_empty()
            && self.shift_spans.is_empty()
            && self.rel_spans.is_empty()
            && self.edge_spans.is_empty()
            && self.align != Align::Justify
            && self.align_last != Some(Align::Justify)
            && !self.text.chars().any(|c| {
                matches!(c, '\t' | SOFT_HYPHEN)
                    || bidi_control(c)
                    || matches!(
                        unicode_bidi::bidi_class(c),
                        unicode_bidi::BidiClass::R | unicode_bidi::BidiClass::AL
                    )
            })
    }

    fn unit_spacing(&self, start: usize, unit: &str) -> Pixels {
        let Some((last, _)) = unit
            .char_indices()
            .rev()
            .find(|(_, c)| c.general_category() != GeneralCategory::Format)
        else {
            return px(0.);
        };
        let at = start + last;
        let mut extra = self
            .letter_spans
            .iter()
            .find(|(r, _)| r.contains(&at))
            .map_or(self.letter_spacing, |(_, v)| *v);
        if unit.chars().next().is_some_and(word_separator) {
            extra += self
                .word_spans
                .iter()
                .find(|(r, _)| r.contains(&start))
                .map_or(self.word_spacing, |(_, v)| *v);
        }
        extra
    }

    pub(crate) fn spacing_before(&self, start: usize, end: usize) -> Pixels {
        let mut extra = px(0.);
        for (off, unit) in self.text[start..]
            .grapheme_indices(true)
            .take_while(|(off, unit)| start + off + unit.len() <= end)
        {
            extra += self.unit_spacing(start + off, unit);
        }
        extra
    }

    /// Apply advances after shaping (Blink line_breaker.cc: ShapeText), so
    /// changing tracking at an inline boundary cannot split a ligature/context.
    pub(crate) fn space_shaped(&self, range: &Range<usize>, shaped: &mut ShapedLine) {
        if !self.joined_spacing() {
            return;
        }
        // `shape` removes default-ignorable controls and adds an LRO/PDF pair.
        // Keep a byte map instead of assuming source and shaped offsets agree.
        let mut map = Vec::new();
        let mut byte = '\u{202d}'.len_utf8();
        for (off, ch) in self.text[range.clone()].char_indices() {
            if !invisible(ch) {
                map.push((byte, range.start + off));
                byte += ch.len_utf8();
            }
        }
        let old: &LineLayout = shaped;
        let mut runs = old.runs.clone();
        for glyph in runs.iter_mut().flat_map(|r| &mut r.glyphs) {
            if let Some((_, at)) = map.iter().rev().find(|(i, _)| *i <= glyph.index) {
                glyph.position.x += self.spacing_before(range.start, *at);
            }
        }
        let extra = self.spacing_before(range.start, range.end);
        **shaped = Arc::new(LineLayout {
            font_size: old.font_size,
            width: old.width + extra,
            ascent: old.ascent,
            descent: old.descent,
            len: old.len,
            runs,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paragraph(text: &str) -> Paragraph {
        Paragraph::new(
            text.to_string().into(),
            vec![],
            px(16.),
            px(16.),
            Align::Left,
            Wrap::default(),
        )
        .letter_spacing(px(4.))
    }

    #[test]
    fn controls_do_not_add_tracking_units() {
        let p = paragraph("a\u{200b}\u{2060}\u{2061}b");
        assert_eq!(p.spacing_before(0, p.text.len()), px(8.));
    }

    #[test]
    fn combining_mark_moves_with_its_base() {
        let p = paragraph("a\u{301}b");
        assert_eq!(p.spacing_before(0, 1), px(0.));
        assert_eq!(p.spacing_before(0, 3), px(4.));
        assert_eq!(p.spacing_before(0, 4), px(8.));
    }

    #[test]
    fn autospace_after_combining_sequence_uses_its_end() {
        let p = paragraph("a\u{301}永").letter_spans(vec![(1..3, px(6.))]);
        assert_eq!(p.spacing_before(0, 3), px(6.));
        assert_eq!(p.spacing_before(0, p.text.len()), px(10.));
    }

    #[test]
    fn different_spacing_paths_do_not_share_measurements() {
        let mut p = paragraph("ab");
        let joined = p.measure_key();
        p.align = Align::Justify;
        assert_ne!(joined, p.measure_key());
    }
}
