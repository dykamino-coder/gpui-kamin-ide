//! Точки переноса по UAX #14 (`unicode_linebreak`) для текста абзаца:
//! распорки полей строчных коробок (`inline::SPACER`) из текста вынимаются,
//! а смещения отображаются обратно; распорка атома читается знаком U+FFFC
//! (класс CB), и у атома разрыв есть с обеих сторон, кроме соседства с
//! GL/WJ/ZWJ (css-text-3 §5.1); `word-break: break-all` подменяет классы
//! букв и цифр на ID перед вызовом переносчика (css-text-3 §5.2).

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
            // Руби — строчная коробка: соседом служит знак её базы, пустая
            // база прозрачна (`text::ruby::base_edges`).
            let prev_of = |mut at: usize| -> Option<char> {
                loop {
                    let (i, ch) = self.text[..at].char_indices().next_back()?;
                    if atom_at(i) {
                        match self.ruby_edges(i) {
                            Some((_, Some(last))) => return Some(last),
                            Some((_, None)) => {}
                            None => return Some('\u{fffc}'),
                        }
                    } else if !skip(i) {
                        return Some(ch);
                    }
                    at = i;
                }
            };
            let next_of = |mut at: usize| -> Option<char> {
                loop {
                    let ch = self.text.get(at..)?.chars().next()?;
                    if atom_at(at) {
                        match self.ruby_edges(at) {
                            Some((Some(first), _)) => return Some(first),
                            Some((None, _)) => {}
                            None => return Some('\u{fffc}'),
                        }
                    } else if !skip(at) {
                        return Some(ch);
                    }
                    at += ch.len_utf8();
                }
            };
            for b in &self.atom_boxes {
                let end = b.at + 3;
                // Края руби — точки UAX #14 между знаками баз и соседями
                // (css-ruby-1 §2.1: руби не атомарна; Blink и WebKit не рвут
                // `<ruby>a<rt>x</rt></ruby>b`); пустая база точек не даёт.
                if let Some((first, last)) = self.ruby_edges(b.at) {
                    let first = first.or_else(|| next_of(end));
                    if let (Some(a), Some(z)) = (prev_of(b.at), first)
                        && pair_breaks(a, z)
                    {
                        out.push((b.at, unicode_linebreak::BreakOpportunity::Allowed));
                    }
                    if let (Some(a), Some(z)) = (last, next_of(end))
                        && pair_breaks(a, z)
                    {
                        out.push((end, unicode_linebreak::BreakOpportunity::Allowed));
                    }
                    continue;
                }
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
                // Руби остаётся распоркой WJ: свои точки она получает от
                // знаков баз (`linebreaks`).
                if t.get(b.at..b.at + 3) == Some("\u{feff}") && self.ruby_edges(b.at).is_none() {
                    t.replace_range(b.at..b.at + 3, "\u{fffc}");
                }
            }
            replaced = t;
            &replaced
        };
        // CSS Text 3 §5.2: break-all tailors letter/number classes to ID,
        // then applies UAX #14 unchanged (including BA, GL and punctuation).
        let tailor = self.wrap.break_all || self.spans.iter().any(|(_, w)| w.break_all);
        if self.spacers.is_empty() && !tailor {
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
            let resolved = if self.wrap_at(at).break_all {
                break_all_class(ch)
            } else {
                ch
            };
            // UAX emits only character boundaries. Interior byte entries need
            // no source correspondence when the tailored UTF-8 length differs.
            map.extend(std::iter::repeat_n(at, resolved.len_utf8() - 1));
            clean.push(resolved);
        }
        map.push(self.text.len());
        unicode_linebreak::linebreaks(&clean)
            .map(|(at, kind)| (map.get(at).copied().unwrap_or(self.text.len()), kind))
            .collect()
    }
}

impl Paragraph {
    /// Base-level edge characters of the ruby container atom at `at`.
    fn ruby_edges(&self, at: usize) -> Option<(Option<char>, Option<char>)> {
        self.atoms
            .iter()
            .find(|a| a.at == at)
            .and_then(|a| a.extents.base_edges)
    }
}

/// Whether UAX #14 allows a break between `a` and `b`.
fn pair_breaks(a: char, b: char) -> bool {
    let pair: String = [a, b].into_iter().collect();
    unicode_linebreak::linebreaks(&pair).any(|(at, _)| at == a.len_utf8())
}

/// Feed the ID class to the UAX iterator without altering the shaped text.
/// AI resolves to AL; Hebrew letters are typographic letter units too.
pub(super) fn break_all_class(ch: char) -> char {
    use unicode_linebreak::BreakClass::*;
    match unicode_linebreak::break_property(ch as u32) {
        Alphabetic | Numeric | ComplexContext | HebrewLetter | Ambiguous => '一',
        _ => ch,
    }
}

#[cfg(test)]
mod ruby_tests {
    /// Ruby edges break like the base text: `<ruby>a<rt>x</rt></ruby>b` has
    /// no opportunity, a space or CJK neighbours do.
    #[test]
    fn ruby_edges_follow_uax14_between_base_and_neighbour() {
        assert!(!super::pair_breaks('a', 'b'));
        assert!(super::pair_breaks(' ', 'a'));
        assert!(super::pair_breaks('漢', '字'));
        assert!(!super::pair_breaks('a', ','));
    }
}
