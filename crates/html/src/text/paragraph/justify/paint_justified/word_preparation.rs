//! Подготовка слов строки к отрисовке по словам (`prepare_words`): разрезы
//! по границам кусков с трекингом, сдвигами и краями коробок, по краям
//! видимых прогонов UAX #9 L2 и распорок; пробелы левее последней
//! табуляции раздачи не получают (css-text-4 §8.1); шаг раздачи остатка на
//! каждый пробел и начало строки.

use super::{Edge, Seg, Word};
use crate::text::paragraph::*;
use gpui::{Pixels, px};

impl Paragraph {
    // Prepared words retain their run ranges and measured paint geometry together.
    #[allow(clippy::type_complexity)]
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
            let cuts = &self.prepared().cuts;
            let mut split: Vec<Word> = Vec::with_capacity(words.len());
            for w in words {
                let mut at = w.range.start;
                let mut spaces = w.spaces_before;
                for cut in cuts[cuts.partition_point(|c| *c < w.range.start)
                    ..cuts.partition_point(|c| *c < w.range.end)]
                    .iter()
                    .copied()
                    .filter(|c| *c > range.start && *c < range.end)
                {
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
            let all_edges = &self.prepared().box_edges;
            let mut edges = all_edges[all_edges.partition_point(|s| *s < range.start)
                ..all_edges.partition_point(|s| *s <= range.end)]
                .to_vec();
            edges.extend(ltr_runs.iter().flat_map(|r| [r.0, r.1]));
            edges.sort_unstable();
            edges.dedup();
            let mut split: Vec<Word> = Vec::with_capacity(words.len());
            for w in words {
                let mut at = w.range.start;
                let first = edges.partition_point(|s| *s <= at);
                let last = edges.partition_point(|s| *s < w.range.end);
                for &c in &edges[first..last] {
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
