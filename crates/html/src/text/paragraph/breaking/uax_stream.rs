//! Поток точек UAX #14 по тексту абзаца без распорок полей: смещения
//! переносчика отображаются обратно на исходный текст в ногу с его точками,
//! без побайтной таблицы на весь абзац (`for_each_linebreak_uax`).

use super::unicode_breaks::break_all_class;
use crate::text::paragraph::*;

impl Paragraph {
    pub(super) fn for_each_linebreak_uax(
        &self,
        mut f: impl FnMut(usize, unicode_linebreak::BreakOpportunity),
    ) {
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
            for (at, kind) in unicode_linebreak::linebreaks(text) {
                f(at, kind);
            }
            return;
        }
        let resolved = |at: usize, ch: char| {
            if tailor && self.wrap_at(at).break_all {
                break_all_class(ch)
            } else {
                ch
            }
        };
        let spacer = |at: usize| self.spacers.binary_search(&at).is_ok();
        let mut clean = String::with_capacity(text.len());
        for (at, ch) in text.char_indices() {
            if !spacer(at) {
                clean.push(resolved(at, ch));
            }
        }
        // Смещения переносчика отображаются обратно тем же проходом по
        // тексту, идущим в ногу с его точками (они возрастают): знак после
        // распорок отвечает началом распорок, байт внутри подменённого знака —
        // началом знака, конец — концом текста. Побайтная таблица на весь
        // абзац для этого не нужна.
        let mut source = text.char_indices();
        let mut pending: Option<usize> = None;
        let mut clean_at = 0usize;
        let mut next = || {
            for (at, ch) in source.by_ref() {
                if spacer(at) {
                    pending.get_or_insert(at);
                    continue;
                }
                let start = clean_at;
                clean_at += resolved(at, ch).len_utf8();
                return Some((start, clean_at, pending.take().unwrap_or(at), at));
            }
            None
        };
        let mut current: Option<(usize, usize, usize, usize)> = None;
        for (at, kind) in unicode_linebreak::linebreaks(&clean) {
            let mapped = loop {
                if let Some((start, end, first, inner)) = current
                    && at < end
                {
                    break if at <= start { first } else { inner };
                }
                match next() {
                    Some(c) => current = Some(c),
                    None => break self.text.len(),
                }
            };
            f(mapped, kind);
        }
    }
}
