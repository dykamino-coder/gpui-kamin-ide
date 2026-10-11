//! Bands for justify; split out to keep the owning module within 250 lines.

use super::Word;
use crate::text::paragraph::*;
use gpui::{Hsla, Pixels, SharedString, TextRun, Window};

impl Paragraph {
    /// Ширина слова в наборе — тем же путём, что и отрисовка.
    /// ПРОБОВАЛИ И ОТКАТИЛИ: чистить слово от незримых знаков перед набором
    /// и замером, как это делает общий путь (`trim_runs`). Проба по 296 парам
    /// семей `bidi-*`, `letter-spacing-*`, `shaping-arabic-*`: не сдвинулась
    /// НИ ОДНА — знаки управления двунаправленностью лежат в своих кусках, а
    /// не внутри слов, и в этот путь не попадают.
    pub(crate) fn word_width(&self, word: &Word, window: &mut Window) -> Pixels {
        let slice: SharedString = self.text[word.range.clone()].to_string().into();
        let runs = slice_runs(&self.runs, &word.range);
        window
            .text_system()
            .with_ligature_breaking(false)
            .shape_line_spaced(
                slice,
                self.font_size,
                &runs,
                None,
                self.letter_spans
                    .iter()
                    .find(|(r, _)| r.contains(&word.range.start))
                    .map(|(_, v)| *v)
                    .unwrap_or(self.letter_spacing),
            )
            .width
    }
}

impl Paragraph {
    /// Подложка промежутка между словами — только если оба соседа и сам
    /// промежуток лежат в ОДНОМ прогоне, и у него есть фон.
    pub(crate) fn gap_background(&self, left: &Word, right: &Word) -> Option<gpui::Hsla> {
        let run_at = |at: usize| -> Option<usize> {
            let mut start = 0usize;
            for (i, run) in self.runs.iter().enumerate() {
                if at < start + run.len {
                    return Some(i);
                }
                start += run.len;
            }
            None
        };
        let a = run_at(left.range.end.saturating_sub(1))?;
        let b = run_at(right.range.start)?;
        let gap = run_at(left.range.end)?;
        if a != b || a != gap {
            return None;
        }
        self.runs[a].background_color
    }
}

impl Paragraph {
    /// Band identity (colour and border) of the run holding byte `at`.
    pub(crate) fn band_at(&self, at: usize) -> Option<(Option<Hsla>, Option<(Hsla, [Pixels; 4])>)> {
        let mut start = 0usize;
        for run in self.runs.iter() {
            if at < start + run.len {
                return Some((run.background_color, run.background_border));
            }
            start += run.len;
        }
        None
    }
}

impl Paragraph {
    /// Runs of a word of a mirrored right-to-left line (no reordered
    /// pieces): the word stands as one unit, its logical successor on its
    /// left and its predecessor on its right. The inline box band goes on
    /// across a side whose neighbour belongs to the same band, and that side
    /// gets no padding or border (css-break-3 §5.4 `box-decoration-break:
    /// slice`; mirror of `slice_runs_banded`).
    pub(crate) fn mirrored_band_runs(&self, word: &std::ops::Range<usize>) -> Vec<TextRun> {
        let left = self.band_at(word.end);
        let right = word.start.checked_sub(1).and_then(|a| self.band_at(a));
        let mut out = slice_runs(&self.runs, word);
        for run in out.iter_mut() {
            if run.background_color.is_none() {
                continue;
            }
            let own = Some((run.background_color, run.background_border));
            cut_band_sides(run, left == own, right == own);
        }
        out
    }
}

impl Paragraph {
    /// Runs of a word placed by visual pieces (`ltr_place`, piece `k`): an
    /// inline box draws a side (padding and border) only where its own edge
    /// spacer is the VISUAL neighbour. A box split by bidi reordering keeps
    /// its left edge on its leftmost fragment and its right edge on the
    /// rightmost one, where the line painter put its spacers; a fragment
    /// continued from or onto another line has no side there (CSS 2.1 §8.6,
    /// css-break-3 §5.4; Blink `NGInlineBoxFragmentPainter` paints sides
    /// per `NGPhysicalBoxFragment::SidesToInclude`).
    pub(crate) fn visual_band_runs(
        &self,
        place: &[(usize, usize, bool, Pixels)],
        k: usize,
        word: &std::ops::Range<usize>,
    ) -> Vec<TextRun> {
        let rtl = place[k].2;
        let vis = self.visual_chars(place.iter().map(|p| (p.0, p.1, p.2)));
        let (left, right) = visual_neighbours(&self.text, &vis, word);
        let mut out = slice_runs(&self.runs, word);
        self.cut_piece_sides(&mut out, word, rtl, left, right);
        out
    }
}

impl Paragraph {
    /// Byte offsets of the characters of a line in visual order, from its
    /// pieces `(start, end, rtl)` in visual order.
    pub(crate) fn visual_chars(
        &self,
        pieces: impl Iterator<Item = (usize, usize, bool)>,
    ) -> Vec<usize> {
        let mut out = Vec::new();
        for (s, e, rtl) in pieces {
            let Some(text) = self.text.get(s..e) else {
                continue;
            };
            let at = out.len();
            out.extend(text.char_indices().map(|(i, _)| s + i));
            if rtl {
                out[at..].reverse();
            }
        }
        out
    }
}
