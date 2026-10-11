//! Unicode breaks for breaking; split out to keep the owning module within 250 lines.

use crate::text::paragraph::*;

impl Paragraph {
    /// Точки переноса по UAX-14 — по тексту БЕЗ знаков-распорок.
    ///
    /// Распорка (`inline::SPACER`) — не знак документа, а место под поля
    /// строчной коробки. Класс WJ запрещает разрыв и перед собой, поэтому
    /// пробел ПЕРЕД `<span>` с отступом переставал быть точкой переноса, и
    /// строка уходила за край коробки вместо переноса. Разрыв возвращается на
    /// место распорки: поле уезжает на новую строку вместе со своим текстом.
    pub(crate) fn linebreaks(&self) -> Vec<(usize, unicode_linebreak::BreakOpportunity)> {
        let mut out = self.linebreaks_uax();
        // Атом — точка переноса с обеих сторон (css-text-3 §5.1: для переноса
        // атом — знак-заместитель объекта; Blink `line_breaker.cc` рвёт до и
        // после атомарной коробки). Нельзя только рядом со знаками GL/WJ/ZWJ
        // — «with the exception of U+00A0 NO-BREAK SPACE» (§5.1
        // «atomic-compat-wrap»): рядом с ним разрыв, наоборот, есть
        // (`line-breaking-atomic-001/002`). Рядом с пробелом точку даёт сам
        // UAX #14 — после ряда пробелов, а не перед ним.
        if !self.atom_boxes.is_empty() {
            let glue = |ch: char| {
                use unicode_linebreak::BreakClass::*;
                ch != '\u{a0}'
                    && matches!(
                        unicode_linebreak::break_property(ch as u32),
                        NonBreakingGlue | WordJoiner | ZeroWidthJoiner
                    )
            };
            // Пунктуация разрыв у атома НЕ гасит: «there is a soft wrap
            // opportunity before and after each replaced element or other
            // atomic inline, even when adjacent to a character that would
            // normally suppress them» (css-text-3 §5.1;
            // `line-breaking-replaced-006`: `<img>:` рвётся перед двоеточием).
            let space = |ch: char| matches!(ch, ' ' | '\t' | '\n' | '\u{200b}');
            // Соседний знак — мимо распорок полей (их перенос не видит, см.
            // `linebreaks_uax`); соседний атом читается знаком-заместителем.
            let atom_at = |at: usize| self.atom_boxes.iter().any(|x| x.at == at);
            let skip = |at: usize| self.spacers.binary_search(&at).is_ok();
            let prev_of = |mut at: usize| -> Option<char> {
                loop {
                    let (i, ch) = self.text[..at].char_indices().next_back()?;
                    if atom_at(i) {
                        return Some('\u{fffc}');
                    }
                    if !skip(i) {
                        return Some(ch);
                    }
                    at = i;
                }
            };
            let next_of = |mut at: usize| -> Option<char> {
                loop {
                    let ch = self.text.get(at..)?.chars().next()?;
                    if atom_at(at) {
                        return Some('\u{fffc}');
                    }
                    if !skip(at) {
                        return Some(ch);
                    }
                    at += ch.len_utf8();
                }
            };
            for b in &self.atom_boxes {
                let end = b.at + 3;
                if let Some(prev) = prev_of(b.at)
                    && !space(prev)
                    && !glue(prev)
                {
                    out.push((b.at, unicode_linebreak::BreakOpportunity::Allowed));
                }
                if let Some(next) = next_of(end)
                    && !space(next)
                    && !glue(next)
                {
                    out.push((end, unicode_linebreak::BreakOpportunity::Allowed));
                }
            }
            out.sort_by_key(|(at, _)| *at);
            out.dedup_by_key(|(at, _)| *at);
        }
        out
    }
}

impl Paragraph {
    pub(crate) fn linebreaks_uax(&self) -> Vec<(usize, unicode_linebreak::BreakOpportunity)> {
        // Распорка атома читается как U+FFFC: класс CB даёт разрыв до и после
        // (UAX #14 LB20; css-text-3 §5.1 — для переноса атом как знак-
        // заместитель объекта, Blink `inline_items_builder.cc`). Длина в
        // UTF-8 у U+FEFF и U+FFFC одна — смещения не съезжают.
        let replaced;
        let text: &str = if self.atom_boxes.is_empty() {
            &self.text
        } else {
            let mut t = self.text.to_string();
            for b in &self.atom_boxes {
                if t.get(b.at..b.at + 3) == Some("\u{feff}") {
                    t.replace_range(b.at..b.at + 3, "\u{fffc}");
                }
            }
            replaced = t;
            &replaced
        };
        if self.spacers.is_empty() {
            return unicode_linebreak::linebreaks(text).collect();
        }
        let mut clean = String::with_capacity(text.len());
        let mut map: Vec<usize> = Vec::with_capacity(text.len() + 1);
        let mut pending: Option<usize> = None;
        for (at, ch) in text.char_indices() {
            if self.spacers.binary_search(&at).is_ok() {
                pending.get_or_insert(at);
                continue;
            }
            map.push(pending.take().unwrap_or(at));
            for k in 1..ch.len_utf8() {
                map.push(at + k);
            }
            clean.push(ch);
        }
        map.push(self.text.len());
        unicode_linebreak::linebreaks(&clean)
            .map(|(at, kind)| (map.get(at).copied().unwrap_or(self.text.len()), kind))
            .collect()
    }
}
