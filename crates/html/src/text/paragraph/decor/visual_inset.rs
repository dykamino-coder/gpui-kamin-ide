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
        let Some((_, runs)) = self.resolved_visual(line, false) else {
            return (0.0, 0.0);
        };
        let (mut before, mut after) = (0.0, 0.0);
        let mut found = false;
        for (start, end, rtl) in runs {
            let a = start.max(group.start);
            let b = end.min(group.end);
            if a >= b {
                continue;
            }
            if a <= s && e <= b {
                let left = if rtl { e..b } else { a..s };
                let right = if rtl { a..s } else { e..b };
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
