//! KaminIDE patch: shaping of runs with a zero em size.
//!
//! `font-size: 0` is a valid computed value (css-fonts-4 §2.5: negative
//! lengths are invalid, zero is not), so its text reaches the backend at em
//! size 0. A text format or range at em size 0 is outside what DirectWrite
//! lays out, and such a line once hung shaping. A zero-size run has zero
//! advances and no ink at any scale, so it is a blank run: Gecko returns a
//! blank text run for a size-0 font group without shaping, Blink shapes it
//! through HarfBuzz at scale 0.
//!
//! A line made only of blank runs is not shaped at all. In a mixed line a
//! blank run keeps its place in the text (bidi, clusters and indices of the
//! other runs are unchanged) and is shaped at a stand-in em size that differs
//! from both neighbours, so DirectWrite gives it glyph runs of its own; the
//! renderer drops those glyph runs, which adds neither glyphs nor advance.

use gpui::Pixels;

/// A run whose em size is zero (or not a positive number) shapes as blank.
pub(super) fn is_blank(size: Pixels) -> bool {
    let size = size.as_f32();
    size.is_nan() || size <= 0.0
}

/// Em size a blank run is shaped at between runs of `prev` and `next` em
/// sizes. Equal sizes (or sizes one ulp apart, the upstream ligature nudge)
/// would let DirectWrite merge the blank run into a neighbour's glyph run.
pub(super) fn stand_in(prev: Option<f32>, next: Option<f32>) -> f32 {
    let close = |a: f32, b: f32| a == b || a.next_up() == b || b.next_up() == a;
    let mut size = 1.0f32;
    // Each neighbour rules out at most one power of two: at most two steps.
    while [prev, next].into_iter().flatten().any(|n| close(n, size)) {
        size *= 2.0;
    }
    size
}

/// Whether the glyph run starting at `position` (UTF-16) belongs to a blank
/// run; a blank run never shares a glyph run with its neighbours.
pub(super) fn in_blank(ranges: &[(u32, u32)], position: u32) -> bool {
    ranges
        .iter()
        .any(|&(start, len)| position >= start && position < start + len)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::px;

    #[test]
    fn zero_and_non_positive_sizes_are_blank() {
        assert!(is_blank(px(0.)));
        assert!(is_blank(px(-0.)));
        assert!(is_blank(px(f32::NAN)));
        assert!(!is_blank(px(0.01)));
        assert!(!is_blank(px(16.)));
    }

    #[test]
    fn stand_in_differs_from_both_neighbours() {
        assert_eq!(stand_in(None, None), 1.0);
        assert_eq!(stand_in(Some(16.), Some(12.)), 1.0);
        assert_eq!(stand_in(Some(1.), None), 2.0);
        assert_eq!(stand_in(Some(1.), Some(2.)), 4.0);
        assert_eq!(stand_in(Some(2.), Some(1.)), 4.0);
        // The upstream ligature nudge moves a size by one ulp.
        assert_eq!(stand_in(Some(1.0f32.next_down()), None), 2.0);
        assert_eq!(stand_in(None, Some(1.0f32.next_up())), 2.0);
    }

    #[test]
    fn glyph_runs_inside_blank_ranges_are_found() {
        let ranges = [(3, 2), (9, 1)];
        assert!(!in_blank(&ranges, 0));
        assert!(in_blank(&ranges, 3));
        assert!(in_blank(&ranges, 4));
        assert!(!in_blank(&ranges, 5));
        assert!(in_blank(&ranges, 9));
        assert!(!in_blank(&ranges, 10));
        assert!(!in_blank(&[], 0));
    }
}
