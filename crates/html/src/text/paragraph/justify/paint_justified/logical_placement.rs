//! Logical placement for paint_justified; split out to keep the owning module within 250 lines.

use super::{Edge, Seg, Word};
use crate::text::paragraph::*;
use gpui::{Pixels, Window, px};

impl Paragraph {
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
                    mirrored
                        .sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
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
