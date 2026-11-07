//! Межбуквенный интервал после переупорядочивания bidi (css-text-3 §8.2: трекинг ставится
//! между ВИЗУАЛЬНО соседними единицами). Буква, оторванная переупорядочиванием (UAX #9 L2) от
//! логического преемника, граничит с буквой другого куска и берёт зазор общего предка —
//! блока; возвращаются только переопределения поверх уже посчитанных диапазонов.

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
    let texts = || {
        pieces.iter().enumerate().filter_map(|(k, p)| match p {
            Piece::Text { text, .. } if !text.is_empty() => Some((k, text)),
            _ => None,
        })
    };
    // Без знаков справа налево переупорядочивания нет: абзац не склеивается.
    let bidi = texts().any(|(_, t)| {
        t.chars().any(|c| {
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
        })
    });
    if !bidi {
        return Vec::new();
    }
    let mut text = String::with_capacity(texts().map(|(_, t)| t.len()).sum());
    let mut piece_of: Vec<(Range<usize>, usize)> = Vec::with_capacity(texts().count());
    for (k, t) in texts() {
        piece_of.push((text.len()..text.len() + t.len(), k));
        text.push_str(t);
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
        // Tracked units (zero-width controls and breaks take no tracking and
        // are skipped, §8.2) in logical order: ascending byte offsets.
        let units = |range: Range<usize>| {
            text[range.clone()]
                .char_indices()
                .filter(|(_, c)| !super::spacing::zero_width_format(*c) && *c != '\n')
                .map(move |(i, _)| range.start + i)
        };
        let logical: Vec<usize> = units(para.range.clone()).collect();
        // Visual sequence of the same units.
        let mut visual: Vec<usize> = Vec::with_capacity(logical.len());
        for run in runs {
            let start = visual.len();
            visual.extend(units(run.clone()));
            if levels[run.start].is_rtl() {
                visual[start..].reverse();
            }
        }
        debug_assert_eq!(
            visual.len(),
            logical.len(),
            "visual runs partition the line"
        );
        // Visual position of each logical unit (offsets are unique and sorted).
        let mut rank = vec![0usize; logical.len()];
        for (p, at) in visual.iter().enumerate() {
            if let Ok(l) = logical.binary_search(at) {
                rank[l] = p;
            }
        }
        for (l, pair) in logical.windows(2).enumerate() {
            let i = pair[0];
            let (pi, pj) = (rank[l], rank[l + 1]);
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
pub fn block_letter_spacing(
    style: &crate::style::computed::Computed,
    size: f32,
    vcx: crate::values::ValueCx<'_>,
) -> Pixels {
    gpui::px(crate::text::metrics::spacing_px(
        style.inh.text_flow.letter_spacing,
        &style.inh.font.font_family.clone().unwrap_or_default(),
        size,
        vcx,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::computed::{BoxStyle, Computed};
    use gpui::px;

    fn text(t: &str) -> Piece {
        Piece::Text {
            text: t.into(),
            style: BoxStyle::from(Computed::default()),
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

    /// Visual neighbours by the definition: units sorted by their visual
    /// position, read through a map from offset to position.
    fn reference(text: &str, rtl: bool) -> Vec<(usize, Option<usize>)> {
        let info = BidiInfo::new(text, Some(if rtl { Level::rtl() } else { Level::ltr() }));
        let mut out = Vec::new();
        for para in &info.paragraphs {
            let (levels, runs) = info.visual_runs(para, para.range.clone());
            let mut visual = Vec::new();
            for run in runs {
                let mut chars: Vec<usize> = text[run.clone()]
                    .char_indices()
                    .map(|(i, _)| run.start + i)
                    .collect();
                if levels[run.start].is_rtl() {
                    chars.reverse();
                }
                visual.extend(chars);
            }
            let order: std::collections::HashMap<usize, usize> =
                visual.iter().enumerate().map(|(p, at)| (*at, p)).collect();
            let mut logical = visual.clone();
            logical.sort_unstable();
            for at in logical {
                let p = order[&at];
                let next = if levels[at].is_rtl() {
                    p.checked_sub(1)
                } else {
                    Some(p + 1)
                };
                out.push((at, next.and_then(|n| visual.get(n).copied())));
            }
        }
        out
    }

    #[test]
    fn overrides_follow_the_visual_order_of_mixed_runs() {
        // One piece per letter: every non-adjacent pair is a piece boundary,
        // so each reordered letter must take the block gap exactly where the
        // reference visual order says its trailing neighbour is not its successor.
        for (text, rtl) in [
            ("ab\u{5d0}\u{5d1}cd", false),
            ("\u{5d0}a1\u{5d1}b", true),
            ("a\u{627}\u{644}2b", false),
        ] {
            let pieces: Vec<Piece> = text
                .chars()
                .map(|c| super::tests::text(&c.to_string()))
                .collect();
            let got = bidi_letter_spans(&pieces, rtl, px(0.), &[(0..text.len(), px(3.))]);
            let want: Vec<(Range<usize>, Pixels)> = reference(text, rtl)
                .windows(2)
                .filter(|w| w[0].1 != Some(w[1].0))
                .map(|w| {
                    (
                        w[0].0..w[0].0 + text[w[0].0..].chars().next().unwrap().len_utf8(),
                        px(0.),
                    )
                })
                .collect();
            assert_eq!(got, want, "{text:?} rtl={rtl}");
        }
    }
}
