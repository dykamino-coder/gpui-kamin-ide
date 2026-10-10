//! Word preparation for paint_justified; split out to keep the owning module within 250 lines.

use super::{Edge, Seg, Word};
use crate::text::paragraph::*;
use gpui::{Pixels, px};

impl Paragraph {
    pub(crate) fn prepare_words(
        &self,
        range: &std::ops::Range<usize>,
        segs: &[Seg],
        free: Pixels,
    ) -> (Vec<Word>, Vec<(usize, usize, bool)>, Pixels, usize, Pixels) {
        let mut words = self.words(range);
        // Слово режется по границам кусков с трекингом: набор принимает
        // трекинг скаляром, поэтому кусок с другим значением обязан идти
        // отдельным вызовом. Без этого `letter-spacing` на `<span>` внутри
        // слова не действовал вовсе.
        if !self.letter_spans.is_empty()
            || !self.shift_spans.is_empty()
            || !self.rel_spans.is_empty()
            || !self.edge_spans.is_empty()
        {
            let mut cuts: Vec<usize> = Vec::new();
            let mut cut = |edge: usize| {
                if edge > range.start && edge < range.end {
                    cuts.push(edge);
                }
            };
            for (r, _) in self.letter_spans.iter() {
                // Трекинг знака — это промежуток ПОСЛЕ него, поэтому у
                // последнего знака отрезка он на набор внутри отрезка не
                // влияет. Резать по началу нужно только там, где знаков в
                // диапазоне несколько: одиночный (зазор `text-autospace`)
                // спокойно доживает в общем отрезке, а свой разрез оставлял
                // между половинками слова шов в точку — соседние отрезки
                // округляются независимо (`text-autospace-001`: `XX`
                // расходились).
                // A box spacer carries its entire advance in tracking (CSS 2.1
                // section 8.3). Joining it to a preceding word discards that
                // advance when the word is shaped with its own spacing.
                if self.text[r.clone()].chars().nth(1).is_some()
                    || self.spacers.binary_search(&r.start).is_ok()
                {
                    cut(r.start);
                }
                cut(r.end);
            }
            // Сдвиг по вертикали — свойство самого глифа: он обязан ехать
            // отдельным вызовом целиком.
            for (r, _) in self.rel_spans.iter() {
                cut(r.start);
                cut(r.end);
            }
            for (r, _) in self.shift_spans.iter() {
                cut(r.start);
                cut(r.end);
            }
            for (r, _, _) in self.edge_spans.iter() {
                cut(r.start);
                cut(r.end);
            }
            cuts.sort_unstable();
            cuts.dedup();
            let mut split: Vec<Word> = Vec::with_capacity(words.len());
            for w in words {
                let mut at = w.range.start;
                let mut spaces = w.spaces_before;
                for cut in cuts.iter().copied().filter(|c| w.range.contains(c)) {
                    split.push(Word {
                        range: at..cut,
                        spaces_before: spaces,
                    });
                    at = cut;
                    spaces = 0;
                }
                split.push(Word {
                    range: at..w.range.end,
                    spaces_before: spaces,
                });
            }
            words = split;
        }
        // UAX #9 L2 in a left-to-right line: the word path shapes and places
        // pieces one by one, so right-to-left level runs (explicit controls,
        // `unicode-bidi`, strong R/AL text) must be reordered here; without it
        // they were painted in logical order (CSS 2.1 §9.10, `bidi-text/*`).
        // Runs come in visual order; a word crossing a run edge is cut there.
        let ltr_runs = self.line_visual_runs(range);
        if !ltr_runs.is_empty() {
            let mut split: Vec<Word> = Vec::with_capacity(words.len());
            for w in words {
                let mut at = w.range.start;
                let mut edges: Vec<usize> = ltr_runs
                    .iter()
                    .flat_map(|r| [r.0, r.1])
                    .chain(
                        self.spacers
                            .iter()
                            .flat_map(|p| [*p, *p + crate::text::inline::SPACER.len()]),
                    )
                    .chain(self.box_extents.iter().flat_map(|b| [b.1, b.2]))
                    .filter(|s| *s > at && *s < w.range.end)
                    .collect();
                edges.sort_unstable();
                edges.dedup();
                for c in edges {
                    split.push(Word {
                        range: at..c,
                        spaces_before: w.spaces_before,
                    });
                    at = c;
                }
                split.push(Word {
                    range: at..w.range.end,
                    spaces_before: w.spaces_before,
                });
            }
            words = split;
        }
        // Растягивается КАЖДЫЙ пробел, а не промежуток между словами: там, где
        // подряд стоят два сохранённых пробела, добавка идёт дважды.
        //
        // Пробелы ЛЕВЕЕ последней табуляции добавки не получают: табуляция
        // доводит строку до своей позиции и всё лишнее место слева от себя
        // поглощает, поэтому позиции табуляции совпадают с нерастянутой
        // строкой (css-text-4 §8.1). Отсюда оба поведения сразу: строка, где
        // все пробелы левее табуляции, не растягивается вовсе
        // (`text-align-justify-tabs-001`, обе коробки обязаны совпасть), а
        // остаток достаётся только пробелам правее (`-002`: их ровно два, и
        // каждый вырастает на пробел).
        let absorbed = self.text[range.clone()]
            .rfind('\u{9}')
            .map_or(0, |at| self.justify_opps(range, range.start + at));
        if absorbed > 0 {
            for w in words.iter_mut() {
                w.spaces_before = w.spaces_before.saturating_sub(absorbed);
            }
        }
        let opportunities = words.last().map(|w| w.spaces_before).unwrap_or(0);
        let step = if opportunities > 0 {
            free / opportunities as f32
        } else {
            px(0.)
        };
        let from = self.x_at(segs, range.start, Edge::Start);
        (words, ltr_runs, step, absorbed, from)
    }
}
