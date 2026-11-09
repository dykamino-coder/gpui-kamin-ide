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
    // An override replaces a letter's tracking with the block's: where every letter
    // already tracks like the block (no `letter-spacing` below it), there is none,
    // and the paragraph is neither scanned nor reordered.
    if existing.iter().all(|(_, v)| *v == block) {
        return Vec::new();
    }
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
    // (end offset, piece index): the texts are concatenated without gaps, so
    // the piece of an offset is the first one ending after it.
    let mut piece_of: Vec<(usize, usize)> = Vec::with_capacity(texts().count());
    for (k, t) in texts() {
        text.push_str(t);
        piece_of.push((text.len(), k));
    }
    let piece = |at: usize| {
        piece_of
            .get(piece_of.partition_point(|(end, _)| *end <= at))
            .map(|(_, k)| *k)
    };
    let current = |at: usize| {
        existing
            .iter()
            .find(|(r, _)| r.contains(&at))
            .map_or(block, |(_, v)| *v)
    };
    // Tracked units: zero-width controls and breaks take no tracking (§8.2).
    let unit = |c: char| !super::spacing::zero_width_format(c) && c != '\n';
    let base = if rtl { Level::rtl() } else { Level::ltr() };
    let info = BidiInfo::new(&text, Some(base));
    let mut out = Vec::new();
    for para in &info.paragraphs {
        let (levels, runs) = info.visual_runs(para, para.range.clone());
        // Logical successor of a unit inside the paragraph.
        let successor = |i: usize| {
            let from = i + text[i..].chars().next().map_or(1, char::len_utf8);
            text[from..para.range.end]
                .char_indices()
                .find(|(_, c)| unit(*c))
                .map(|(o, _)| from + o)
        };
        // Inside one level run logically consecutive units are visual
        // neighbours (reversed together in an RTL run), so only the logical
        // last unit of a run can be separated from its successor. Its
        // trailing side faces the next visual run (LTR) or the previous one
        // (RTL): the runs are walked in visual order, one pair at a time,
        // without per-unit position tables.
        let mut check = |i: usize, k: Option<usize>| {
            let Some(j) = successor(i) else { return };
            if k == Some(j) {
                return;
            }
            if let Some(k) = k
                && piece(k) == piece(i)
            {
                return;
            }
            if block != current(i) {
                let len = text[i..].chars().next().map_or(1, char::len_utf8);
                out.push((i..i + len, block));
            }
        };
        // (logical first, logical last, rtl) of each run with tracked units.
        let ends = runs.iter().filter_map(|run| {
            let mut units = text[run.clone()]
                .char_indices()
                .filter(|(_, c)| unit(*c))
                .map(|(o, _)| run.start + o);
            let first = units.next()?;
            Some((
                first,
                units.next_back().unwrap_or(first),
                levels[run.start].is_rtl(),
            ))
        });
        let visual_first =
            |(first, last, rtl): (usize, usize, bool)| if rtl { last } else { first };
        let visual_last = |(first, last, rtl): (usize, usize, bool)| if rtl { first } else { last };
        let mut prev: Option<(usize, usize, bool)> = None;
        for run in ends {
            let (_, last, run_rtl) = run;
            if let Some(a) = prev {
                if !a.2 {
                    check(a.1, Some(visual_first(run)));
                }
            }
            if run_rtl {
                check(last, prev.map(visual_last));
            }
            prev = Some(run);
        }
        if let Some((_, last, false)) = prev {
            check(last, None);
        }
    }
    out.sort_unstable_by_key(|(r, _)| r.start);
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
mod tests;
