//! Min content for measure; split out to keep the owning module within 250 lines.

use crate::text::paragraph::*;
use gpui::{Pixels, Window, px};

impl Paragraph {
    /// Ширина по минимальному содержимому — самый широкий кусок, который
    /// разорвать нельзя.
    ///
    /// Нужна раскладке: под неё она меряет высоту, когда ширина ещё не
    /// решена. Ноль тут не годится — по нулю строка рвётся на каждом знаке, и
    /// коробка выходит во много раз выше настоящей.
    pub(crate) fn min_content(&self, window: &mut Window) -> Pixels {
        self.min_content_with(None, window)
    }
}

impl Paragraph {
    /// Min-content contribution with `text-indent` (css-text-3 §7.1, CSS 2.1
    /// §16.1): the content is broken at EVERY soft wrap opportunity, and the
    /// indent (percentages count as zero) is added to the first piece of each
    /// indented line — a negative indent makes that piece narrower
    /// (`text-indent-intrinsic-003/004`, «negative-intrinsic-min»).
    pub(crate) fn min_content_indented(&self, window: &mut Window) -> Pixels {
        self.min_content_with(Some(px(self.indent.px)), window)
    }
}

impl Paragraph {
    pub(crate) fn min_content_with(&self, indent: Option<Pixels>, window: &mut Window) -> Pixels {
        let segs = self.measure(window);
        let mut best = px(0.);
        let mut start = 0usize;
        // Starts a line that carries the indent (`each-line`: also after a
        // forced break).
        let mut indented = indent.is_some();
        let each_line = self.indent.each_line;
        let mut chunk = |from: usize, to: usize, this: &Self, lead: Pixels| {
            let end = if this.spaces_are_content() {
                to
            } else {
                this.hang_tail(from, to)
            };
            // Трекинг ПОСЛЕДНЕГО знака куска на конце строки не действует
            // (css-text-3 §8.2) — `lay_in` его вычитает, а минимум по
            // содержимому считал, и кусок выходил шире на `letter-spacing`.
            let w = this.span(&segs, from, end) - this.tail_spacing(end) + lead;
            let w = if w < px(0.) { px(0.) } else { w };
            if w > best {
                best = w;
            }
        };
        // `overflow-wrap: anywhere` — единственное из семейства, что меняет
        // размер по минимальному содержимому: слово рвётся и здесь, поэтому
        // точками счёта становятся ВСЕ границы знаков (css-text-3 §5.5).
        let stops: Vec<Stop> = if self.wrap.wrap_anywhere {
            self.text
                .char_indices()
                .skip(1)
                .map(|(at, _)| Stop {
                    at,
                    mandatory: false,
                })
                .collect()
        } else {
            // Куски со своим `overflow-wrap: anywhere` добавляют границы
            // знаков только внутри себя.
            let mut stops = self.opportunities();
            for (range, w) in &self.spans {
                if !w.wrap_anywhere {
                    continue;
                }
                for (i, _) in self.text[range.clone()].char_indices().skip(1) {
                    stops.push(Stop {
                        at: range.start + i,
                        mandatory: false,
                    });
                }
            }
            stops.sort_by_key(|s| (s.at, !s.mandatory));
            stops.dedup_by_key(|s| s.at);
            stops
        };
        let lead = |on: bool| if on { indent.unwrap_or(px(0.)) } else { px(0.) };
        for stop in stops {
            if stop.at <= start {
                continue;
            }
            chunk(start, stop.at, self, lead(indented));
            indented = indent.is_some() && each_line && stop.mandatory;
            start = stop.at;
        }
        chunk(start, self.text.len(), self, lead(indented));
        best
    }
}
