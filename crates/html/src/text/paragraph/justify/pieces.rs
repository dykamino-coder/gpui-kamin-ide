//! Pieces for justify; split out to keep the owning module within 250 lines.

use super::{Word, word_separator};
use crate::text::paragraph::*;
use gpui::TextRun;

impl Paragraph {
    /// Cut the band sides at the visual edges of a piece `range` (its runs
    /// `out`, in logical order; `rtl` when its glyphs run right to left)
    /// whose visual neighbours are the bytes `left` and `right`.
    ///
    /// A box with edge spacers (`box_extents`) draws a side only next to its
    /// own spacer: the spacer is where its padding and border sit, so any
    /// other neighbour — another fragment of a box split by bidi reordering,
    /// a segment separator, the start or end of a continued line — means the
    /// box goes on (CSS 2.1 §8.6, css-break-3 §5.4 `box-decoration-break:
    /// slice`). A band without spacers (an outline) goes on only into the
    /// same band, as in `slice_runs_banded`.
    pub(crate) fn cut_piece_sides(
        &self,
        out: &mut [TextRun],
        range: &std::ops::Range<usize>,
        rtl: bool,
        left: Option<usize>,
        right: Option<usize>,
    ) {
        let n = out.len();
        if n == 0 {
            return;
        }
        // Byte offsets of the runs in `out`.
        let mut starts = Vec::with_capacity(n);
        let mut at = range.start;
        for r in out.iter() {
            starts.push(at);
            at += r.len;
        }
        let (li, ri) = if rtl { (n - 1, 0) } else { (0, n - 1) };
        // A piece of zero-width bidi controls only (the marks of `direction`
        // and `unicode-bidi`) has no extent of its own to frame.
        let ghost = self
            .text
            .get(range.clone())
            .is_some_and(|t| t.chars().all(bidi_control));
        let mut cuts: Vec<(usize, bool)> = Vec::new();
        for (idx, nb, is_left) in [(li, left, true), (ri, right, false)] {
            let edge = &out[idx];
            if edge.background_color.is_none() {
                continue;
            }
            let own = (edge.background_color, edge.background_border);
            let pos = starts[idx];
            let holders: Vec<u32> = self
                .box_extents
                .iter()
                .filter(|b| b.1 <= pos && pos < b.2)
                .map(|b| b.0)
                .collect();
            let cut = if ghost {
                true
            } else if holders.is_empty() {
                nb.is_some_and(|a| self.band_at(a) == Some(own))
            } else {
                !nb.is_some_and(|a| {
                    self.spacer_edges.iter().any(|&(p, id, _, _)| {
                        p <= a && a < p + crate::text::inline::SPACER.len() && holders.contains(&id)
                    })
                })
            };
            if !cut {
                continue;
            }
            // The whole band segment touching that edge: `vendor/gpui`
            // draws a band with the sides of its first run.
            let step: isize = if idx == 0 { 1 } else { -1 };
            let mut i = idx as isize;
            while i >= 0 && (i as usize) < n {
                let r = &out[i as usize];
                if (r.background_color, r.background_border) != own {
                    break;
                }
                cuts.push((i as usize, is_left));
                i += step;
            }
        }
        for (i, is_left) in cuts {
            cut_band_sides(&mut out[i], is_left, !is_left);
        }
    }
}

impl Paragraph {
    /// Слова строки — куски между пробелами, каждое со счётом пробелов слева.
    /// Visual level runs `(start, end, rtl)` of a line (UAX #9 L1–L2), or none
    /// when the plain path suffices: a left-to-right line without a
    /// right-to-left run, a right-to-left line of one right-to-left run and no
    /// inline box edges (the mirror below places it).
    ///
    /// Box spacers (`inline::SPACER`, U+FEFF) are analysed as neutrals
    /// (U+FFFC, same UTF-8 length): as boundary neutrals X9 would give a
    /// trailing spacer the level of the embedding it follows, while the edges
    /// belong to the parent's level (CSS Writing Modes 4 §2.4).
    pub(crate) fn line_visual_runs(
        &self,
        range: &std::ops::Range<usize>,
    ) -> Vec<(usize, usize, bool)> {
        if range.start >= range.end || range.end > self.text.len() {
            return Vec::new();
        }
        let rtl = self.wrap.rtl;
        let edges = rtl && self.spacer_edges.iter().any(|e| range.contains(&e.0));
        let needs = edges
            || self.text[range.clone()].chars().any(|c| {
                matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
                    || match unicode_bidi::bidi_class(c) {
                        unicode_bidi::BidiClass::R | unicode_bidi::BidiClass::AL => !rtl,
                        unicode_bidi::BidiClass::AN => true,
                        unicode_bidi::BidiClass::L | unicode_bidi::BidiClass::EN => rtl,
                        _ => false,
                    }
            });
        if !needs {
            return Vec::new();
        }
        let mut text: String = self.text.to_string();
        for &at in &self.spacers {
            if text.get(at..at + 3) == Some("\u{feff}") {
                text.replace_range(at..at + 3, "\u{fffc}");
            }
        }
        let forced = if self.plaintext.is_some() && !rtl {
            None
        } else if rtl {
            Some(unicode_bidi::Level::rtl())
        } else {
            Some(unicode_bidi::Level::ltr())
        };
        let info = unicode_bidi::BidiInfo::new(&text, forced);
        let Some(para) = info
            .paragraphs
            .iter()
            .find(|p| p.range.start <= range.start && range.start < p.range.end)
        else {
            return Vec::new();
        };
        if para.level.is_rtl() != rtl {
            return Vec::new();
        }
        let (levels, visual) = info.visual_runs(para, range.clone());
        let runs: Vec<(usize, usize, bool)> = visual
            .into_iter()
            .map(|r| {
                (
                    r.start,
                    r.end,
                    levels.get(r.start).is_some_and(|l| l.is_rtl()),
                )
            })
            .collect();
        let mixed = if rtl {
            runs.iter().any(|r| !r.2)
        } else {
            runs.iter().any(|r| r.2)
        };
        if mixed || edges { runs } else { Vec::new() }
    }
}

impl Paragraph {
    pub(crate) fn words(&self, range: &std::ops::Range<usize>) -> Vec<Word> {
        let mut out: Vec<Word> = Vec::new();
        let mut start = None;
        let mut spaces = 0usize;
        for (i, ch) in self.text[range.clone()].char_indices() {
            let at = range.start + i;
            // A character-level opportunity (`justify_boundary`) also ends a
            // word: the next unit is placed with one more expansion.
            if let Some(s) = start
                && at > s
                && self.justify_boundary(range.start, at)
            {
                out.push(Word {
                    range: s..at,
                    spaces_before: spaces,
                });
                spaces += 1;
                start = Some(at);
            }
            // Разделитель слов для выключки — не любой пробел. По css-text-3
            // это пробел, неразрывный и идеографический; ТАБУЛЯЦИЯ в него не
            // входит: она доводит строку до своей позиции, и растягивать её
            // нечем. Пока табуляция раскрывалась в пробелы и каждый считался
            // точкой раздачи, остаток размазывался по ней вместо слов.
            if word_separator(ch) {
                if let Some(s) = start.take() {
                    out.push(Word {
                        range: s..at,
                        spaces_before: spaces,
                    });
                }
                spaces += usize::from(!self.ruby_justify);
            } else if ch == '\u{9}' {
                // Табуляция — ГРАНИЦА слова, хотя точкой раздачи и не служит.
                // Её продвижение задаёт позиция табуляции (`Seg::offset`), и
                // внутри слова оно пропадало: строка без пробелов уходила в
                // набор одним куском, и табуляция рисовалась глифом шрифта
                // (`text-indent-tab-positions-001`: `a⇥b⇥c` выходило `abc`).
                if let Some(s) = start.take() {
                    out.push(Word {
                        range: s..at,
                        spaces_before: spaces,
                    });
                }
            } else if start.is_none() {
                start = Some(at);
            }
        }
        if let Some(s) = start {
            out.push(Word {
                range: s..range.end,
                spaces_before: spaces,
            });
        }
        out
    }
}
