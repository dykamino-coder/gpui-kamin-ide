//! Letter spacing across bidi reordering (css-text-3 §8.2: tracking is
//! applied after bidi reordering, between VISUALLY adjacent units).

use crate::text::inline::*;
use gpui::Pixels;
use std::ops::Range;
use unicode_bidi::{BidiClass, BidiInfo, Level};

/// Overrides for the tracking of letters whose logical successor is not their
/// visual neighbour once the paragraph is reordered (UAX #9 L2).
///
/// Our tracking is stored as "space after each letter" in logical order, with
/// the gap at an inline boundary taken from the innermost box containing both
/// letters. Reordering separates a letter from its logical successor: the gap
/// it then borders lies between letters of different pieces, and comes from
/// their common ancestor, approximated by the block (`letter-spacing-bidi-001`:
/// `a<span>bא</span>ב` has no gap at all, though `b` and `א` share a span).
/// Pieces are reordered as one line, which also keeps a box split by
/// reordering in separate fragments after a line break (`-bidi-002`).
pub fn bidi_letter_spans(
    pieces: &[Piece],
    rtl: bool,
    block: Pixels,
    existing: &[(Range<usize>, Pixels)],
) -> Vec<(Range<usize>, Pixels)> {
    let mut text = String::new();
    let mut piece_of: Vec<(Range<usize>, usize)> = Vec::new();
    for (k, p) in pieces.iter().enumerate() {
        if let Piece::Text { text: t, .. } = p
            && !t.is_empty()
        {
            piece_of.push((text.len()..text.len() + t.len(), k));
            text.push_str(t);
        }
    }
    let bidi = text.chars().any(|c| {
        matches!(
            unicode_bidi::bidi_class(c),
            BidiClass::R
                | BidiClass::AL
                | BidiClass::AN
                | BidiClass::RLE
                | BidiClass::RLO
                | BidiClass::RLI
                | BidiClass::LRO
                | BidiClass::LRE
        )
    });
    if !bidi {
        return Vec::new();
    }
    let piece = |at: usize| {
        piece_of
            .iter()
            .find(|(r, _)| r.contains(&at))
            .map(|(_, k)| *k)
    };
    let current = |at: usize| {
        existing
            .iter()
            .find(|(r, _)| r.contains(&at))
            .map_or(block, |(_, v)| *v)
    };
    let base = if rtl { Level::rtl() } else { Level::ltr() };
    let info = BidiInfo::new(&text, Some(base));
    let mut out = Vec::new();
    for para in &info.paragraphs {
        let (levels, runs) = info.visual_runs(para, para.range.clone());
        // Visual sequence of tracked units (zero-width controls and breaks
        // take no tracking and are skipped, §8.2).
        let mut visual: Vec<usize> = Vec::new();
        for run in runs {
            let mut chars: Vec<usize> = text[run.clone()]
                .char_indices()
                .filter(|(_, c)| !super::spacing::zero_width_format(*c) && *c != '\n')
                .map(|(i, _)| run.start + i)
                .collect();
            if levels[run.start].is_rtl() {
                chars.reverse();
            }
            visual.extend(chars);
        }
        let mut logical = visual.clone();
        logical.sort_unstable();
        let order: std::collections::HashMap<usize, usize> =
            visual.iter().enumerate().map(|(p, at)| (*at, p)).collect();
        let pos = |at: usize| order.get(&at).copied();
        for pair in logical.windows(2) {
            let (i, j) = (pair[0], pair[1]);
            let (Some(pi), Some(pj)) = (pos(i), pos(j)) else {
                continue;
            };
            let rtl_i = levels[i].is_rtl();
            let adjacent = if rtl_i { pj + 1 == pi } else { pi + 1 == pj };
            if adjacent {
                continue;
            }
            // The letter's trailing side faces this visual neighbour.
            let k = if rtl_i {
                pi.checked_sub(1).map(|p| visual[p])
            } else {
                visual.get(pi + 1).copied()
            };
            let value = match k {
                Some(k) if piece(k) == piece(i) => continue,
                _ => block,
            };
            if value != current(i) {
                let len = text[i..].chars().next().map_or(1, char::len_utf8);
                out.push((i..i + len, value));
            }
        }
    }
    out
}

/// Block tracking in pixels, resolved like the paragraph's own value.
pub fn block_letter_spacing(style: &crate::style::computed::Computed, size: f32) -> Pixels {
    gpui::px(crate::text::metrics::spacing_px(
        style.letter_spacing,
        &style.font_family.clone().unwrap_or_default(),
        size,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::computed::Computed;
    use gpui::px;

    fn text(t: &str) -> Piece {
        Piece::Text {
            text: t.into(),
            style: Computed::default(),
        }
    }

    #[test]
    fn letter_split_from_its_successor_by_reordering_takes_block_gap() {
        // a<span>bA</span>B with Hebrew A/B: visually a b B A.
        let pieces = [text("a"), text("b\u{5d0}"), text("\u{5d1}")];
        let existing = [(1..4, px(10.))];
        let out = bidi_letter_spans(&pieces, false, px(0.), &existing);
        assert_eq!(out, vec![(1..2, px(0.))]);
    }

    #[test]
    fn uniform_or_unidirectional_text_needs_no_override() {
        let pieces = [text("ab"), text("cd")];
        assert!(bidi_letter_spans(&pieces, false, px(0.), &[(0..4, px(5.))]).is_empty());
        let pieces = [text("a"), text("b\u{5d0}"), text("\u{5d1}")];
        assert!(bidi_letter_spans(&pieces, false, px(5.), &[]).is_empty());
    }
}
