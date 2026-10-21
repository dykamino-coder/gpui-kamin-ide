//! Продвижение оформляющей коробки по физическим сторонам нарисованного
//! куска (css-text-decor-4 §4.1): сколько оформленного текста лежит левее и
//! правее куска в ВИДИМОМ порядке bidi-прогонов строки — логический порядок
//! байтов не годится, когда коробка пересекает несколько прогонов.

use super::*;
use std::ops::Range;

impl Paragraph {
    /// Advance on either physical side of a painted piece. Logical byte order
    /// is insufficient when a decorating box crosses several bidi runs.
    pub(super) fn decor_visual_sides(
        &self,
        segs: &[Seg],
        group: &Range<usize>,
        line: &Range<usize>,
        s: usize,
        e: usize,
    ) -> (f32, f32) {
        let base = if self.wrap.rtl {
            unicode_bidi::Level::rtl()
        } else {
            unicode_bidi::Level::ltr()
        };
        let info = unicode_bidi::BidiInfo::new(
            &self.text,
            if self.plaintext.is_some() {
                None
            } else {
                Some(base)
            },
        );
        let Some(para) = info
            .paragraphs
            .iter()
            .find(|p| p.range.start <= line.start && line.start < p.range.end)
        else {
            return (0.0, 0.0);
        };
        let (levels, runs) = info.visual_runs(para, line.clone());
        let (mut before, mut after) = (0.0, 0.0);
        let mut found = false;
        for run in runs {
            let a = run.start.max(group.start);
            let b = run.end.min(group.end);
            if a >= b {
                continue;
            }
            if a <= s && e <= b {
                let left = if levels[run.start].is_rtl() {
                    e..b
                } else {
                    a..s
                };
                let right = if levels[run.start].is_rtl() {
                    a..s
                } else {
                    e..b
                };
                before += self.decor_width(segs, left.start, left.end);
                after += self.decor_width(segs, right.start, right.end);
                found = true;
            } else if found {
                after += self.decor_width(segs, a, b);
            } else {
                before += self.decor_width(segs, a, b);
            }
        }
        if found {
            (before, after)
        } else {
            (
                self.decor_width(segs, group.start.max(line.start), s),
                self.decor_width(segs, e, group.end.min(line.end)),
            )
        }
    }
}
