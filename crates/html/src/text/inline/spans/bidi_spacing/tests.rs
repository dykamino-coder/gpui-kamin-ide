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
    // Tracking equal to the block gap everywhere: nothing to override.
    assert!(bidi_letter_spans(&pieces, false, px(5.), &[(0..6, px(5.))]).is_empty());
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

#[test]
fn overrides_of_multi_letter_pieces_match_the_visual_reference() {
    // Pieces of several letters: a reordered letter keeps its gap when its
    // trailing visual neighbour is in the same piece, else takes the block.
    for (parts, rtl) in [
        (vec!["ab\u{5d0}", "\u{5d1}c", "d\u{5d2}\u{5d3}e"], false),
        (vec!["\u{5d0}a", "b1\u{5d1}", "\u{5d2} cd"], true),
        (vec!["x\u{627}\u{644}", "2b", "\u{644}y"], false),
    ] {
        let pieces: Vec<Piece> = parts.iter().map(|p| text(p)).collect();
        let joined: String = parts.concat();
        let mut piece_at = Vec::new();
        for (k, p) in parts.iter().enumerate() {
            piece_at.extend(std::iter::repeat_n(k, p.len()));
        }
        let existing = [(0..joined.len(), px(3.))];
        let got = bidi_letter_spans(&pieces, rtl, px(0.), &existing);
        let units = reference(&joined, rtl);
        let want: Vec<(Range<usize>, Pixels)> = units
            .windows(2)
            .filter(|w| {
                let (i, k) = w[0];
                w[1].0 != k.unwrap_or(usize::MAX) && k.is_none_or(|k| piece_at[k] != piece_at[i])
            })
            .map(|w| {
                (
                    w[0].0..w[0].0 + joined[w[0].0..].chars().next().unwrap().len_utf8(),
                    px(0.),
                )
            })
            .collect();
        assert_eq!(got, want, "{parts:?} rtl={rtl}");
    }
}
