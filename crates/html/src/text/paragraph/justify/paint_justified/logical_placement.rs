//! Места слов ltr-прогонов внутри зеркальной rtl-строки: зеркало строки
//! переворачивает ВСЕ слова разом, и латинский прогон выходил задом
//! наперёд; прогоны левого уровня (UAX #9 L2) выкладываются заново в своём
//! порядке на своём месте с теми же промежутками.

use super::{Edge, Seg, Word};
use crate::text::paragraph::*;
use gpui::{Pixels, Window, px};

impl Paragraph {
    // Logical placement combines bidi edges, word geometry and paint context.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn place_logical(
        &self,
        range: &std::ops::Range<usize>,
        segs: &[Seg],
        words: &[Word],
        ltr_place: &[(usize, usize, bool, Pixels)],
        step: Pixels,
        from: Pixels,
        mirror: Pixels,
        window: &mut Window,
    ) -> Vec<(usize, Pixels)> {
        let mut logical_run: Vec<(usize, Pixels)> = vec![];
        if self.wrap.rtl
            && ltr_place.is_empty()
            && range.start < range.end
            && range.end <= self.text.len()
        {
            let info = unicode_bidi::BidiInfo::new(&self.text, Some(unicode_bidi::Level::rtl()));
            if let Some(para) = info
                .paragraphs
                .iter()
                .find(|p| p.range.start <= range.start && range.start < p.range.end)
                .or_else(|| info.paragraphs.first())
            {
                let (levels, visual) = info.visual_runs(para, range.clone());
                for run in visual {
                    if levels.get(run.start).is_some_and(|l| l.is_rtl()) {
                        continue;
                    }
                    let idx: Vec<usize> = words
                        .iter()
                        .enumerate()
                        .filter(|(_, w)| w.range.start >= run.start && w.range.start < run.end)
                        .map(|(i, _)| i)
                        .collect();
                    if idx.len() < 2 {
                        continue;
                    }
                    // Зеркальные места прогона: слева лежит ПОСЛЕДНЕЕ слово.
                    let mut mirrored: Vec<(usize, Pixels, Pixels)> = idx
                        .iter()
                        .map(|&i| {
                            let w = &words[i];
                            let width = self.word_width(w, window);
                            let logical = (self.x_at(segs, w.range.start, Edge::Start) - from)
                                + step * w.spaces_before as f32;
                            let x = mirror - logical - width;
                            (i, x, width)
                        })
                        .collect();
                    mirrored.sort_by(|a, b| f32::from(a.1).total_cmp(&f32::from(b.1)));
                    // Промежутки между зеркальными соседями: в логическом
                    // порядке те же самые, только в обратную сторону.
                    let gaps: Vec<Pixels> = mirrored
                        .windows(2)
                        .map(|p| p[1].1 - (p[0].1 + p[0].2))
                        .rev()
                        .collect();
                    let mut cursor = mirrored[0].1;
                    for (k, &i) in idx.iter().enumerate() {
                        let width = mirrored
                            .iter()
                            .find(|(j, _, _)| *j == i)
                            .map(|(_, _, w)| *w)
                            .unwrap_or(px(0.));
                        logical_run.push((i, cursor));
                        cursor += width + gaps.get(k).copied().unwrap_or(px(0.));
                    }
                }
            }
        }
        logical_run
    }
}
