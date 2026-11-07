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
            let atom_at = |at: usize| {
                crate::work::add!(ParagraphAtomQueries, 1);
                self.prepared().atoms.first(at).is_some()
            };
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
    /// Точки `linebreaks` по порядку, без промежуточного списка там, где
    /// атомов нет: разрывы UAX #14 идут прямо потребителю.
    pub(crate) fn for_each_linebreak(
        &self,
        mut f: impl FnMut(usize, unicode_linebreak::BreakOpportunity),
    ) {
        if self.atom_boxes.is_empty() {
            self.for_each_linebreak_uax(f);
        } else {
            for (at, kind) in self.linebreaks() {
                f(at, kind);
            }
        }
    }

    pub(crate) fn linebreaks_uax(&self) -> Vec<(usize, unicode_linebreak::BreakOpportunity)> {
        let mut out = Vec::new();
        self.for_each_linebreak_uax(|at, kind| out.push((at, kind)));
        out
    }
}

impl Paragraph {
    /// Base-level edge characters of the ruby container atom at `at`.
    fn ruby_edges(&self, at: usize) -> Option<(Option<char>, Option<char>)> {
        crate::work::add!(ParagraphRubyQueries, 1);
        if self.atoms.is_empty() {
            // Щуп замера имеет коробки, но не живые элементы: как и прежде,
            // его распорки не получают семантику руби из чужого абзаца.
            return None;
        }
        // lay_atoms строит коробки в порядке слотов; индекс сохраняет первое
        // попадание даже у неупорядоченных позиций и дубликатов.
        self.prepared()
            .atoms
            .first(at)
            .and_then(|i| self.atoms.get(i))
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
#[path = "unicode_breaks_tests.rs"]
mod tests;
