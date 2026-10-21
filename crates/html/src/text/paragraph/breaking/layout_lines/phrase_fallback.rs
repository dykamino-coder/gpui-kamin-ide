//! Запасной перенос при `word-break: auto-phrase`: мягкие переносы внутри
//! фразы возвращаются только когда неразорванное слово не влезает в строку;
//! берётся последняя влезающая точка, без неё — первая.
use crate::text::paragraph::*;
use gpui::Pixels;

impl Paragraph {
    pub(crate) fn phrase_hyphen_cut(
        &self,
        start: usize,
        end: usize,
        limit: Option<Pixels>,
        segs: &[Seg],
    ) -> Option<usize> {
        if !self.wrap.auto_phrase && !self.spans.iter().any(|(_, w)| w.auto_phrase) {
            return None;
        }
        let mut first = None;
        let mut fitted = None;
        for stop in self.opportunities_with_fallback(true) {
            if stop.at <= start
                || stop.at >= end
                || !self.wrap_at(stop.at - 1).auto_phrase
                || !self.text[..stop.at].ends_with(SOFT_HYPHEN)
            {
                continue;
            }
            first.get_or_insert(stop.at);
            if self.candidate_width(start, stop.at, end, segs, limit).2 {
                break;
            }
            fitted = Some(stop.at);
        }
        fitted.or(first)
    }
}
