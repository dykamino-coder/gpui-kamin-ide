//! Cut for breaking; split out to keep the owning module within 250 lines.

use super::{cluster_edge, no_break_after, no_break_before};
use crate::text::paragraph::*;
use gpui::Pixels;

impl Paragraph {
    /// Ширина строки перед ПРИНУДИТЕЛЬНЫМ разрывом (конец блока — тоже он) с
    /// учётом условного висения, css-text-3 §4.1.3 шаг 4: «If white-space is
    /// set to pre-wrap, the UA must (unconditionally) hang this sequence,
    /// unless the sequence is followed by a forced line break, in which case
    /// it must conditionally hang the sequence instead». Условно висящее
    /// входит в ширину, пока влезает. Висящие без условий знаки перед ним
    /// (U+3000 при `normal`) висят, только если условный ряд начинается уже
    /// НЕ раньше края (`hanging-whitespace-003`: строки с рядом от 6, 5 и 4ch
    /// в коробке 4ch висят целиком, ряд от 3ch занимает место).
    pub(crate) fn conditional_width(
        &self,
        segs: &[Seg],
        head: usize,
        end: usize,
        bare: Pixels,
        room: Option<Pixels>,
    ) -> Pixels {
        let Some(room) = room else { return bare };
        if self.spaces_are_content() || head >= end {
            return bare;
        }
        let body = self.text[head..end]
            .trim_end_matches(['\n', '\u{b}', '\u{c}', '\u{85}', '\u{2028}', '\u{2029}']);
        let body_end = head + body.len();
        // Начало хвостового ряда СОХРАНЁННЫХ пробелов переносящего куска.
        let mut from = body_end;
        for (i, ch) in body.char_indices().rev() {
            let w = self.wrap_at(head + i);
            if matches!(ch, ' ' | '\t') && w.keep_spaces && !w.nowrap && !w.break_spaces {
                from = head + i;
            } else {
                break;
            }
        }
        if from == body_end || self.span(segs, head, from) >= room {
            return bare;
        }
        let full = self.span(segs, head, body_end) - self.tail_spacing(body_end);
        let fit = if full < room { full } else { room };
        if fit > bare { fit } else { bare }
    }
}

impl Paragraph {
    /// Место разрыва внутри неразрывного куска — по знакам, до последнего
    /// влезающего.
    pub(crate) fn cut_by_char(
        &self,
        start: usize,
        end: usize,
        limit: Option<Pixels>,
        x: &dyn Fn(usize) -> Pixels,
    ) -> usize {
        let Some(limit) = limit else { return end };
        let from = x(start);
        let mut last = start;
        // Конец отрезка — тоже граница знака, и проверять его ОБЯЗАТЕЛЬНО:
        // без него разрез, у которого не влезал только последний знак,
        // возвращал весь отрезок целиком. При `break-spaces` это съедало
        // ведущий пробел следующей строки — он уезжал в конец предыдущей.
        let bounds = self.text[start..end]
            .char_indices()
            .map(|(i, _)| start + i)
            .chain(std::iter::once(end));
        for at in bounds {
            if at == start {
                continue;
            }
            // Рвать ВНУТРИ грозди знаков нельзя: огласовка, соединитель и
            // знак вариации принадлежат своей букве и в другую строку не
            // уходят (`overflow-wrap-cluster`: देवनागरी рвалась пополам).
            if !cluster_edge(&self.text, at) {
                continue;
            }
            // И только там, где аварийный разрыв РАЗРЕШЁН: у соседнего куска
            // правила могут быть другими.
            if !self.emergency_ok(at.saturating_sub(1)) && !self.emergency_ok(at) {
                continue;
            }
            // `word-break: break-all` рвёт между БУКВАМИ и запретов типографики
            // не отменяет: перед точкой и после знака-приставки строка не
            // рвётся даже в аварийном разрезе (`word-break-break-all-inline-008`
            // — «X» и «.» обязаны остаться вместе и вылезти за коробку).
            // Семейство `anywhere` — наоборот, рвёт где угодно.
            if !self.loose_at(at.saturating_sub(1)) && !self.loose_at(at) {
                let after = self.text[at..].chars().next();
                let before = self.text[..at].chars().next_back();
                if after.is_some_and(no_break_before) || before.is_some_and(no_break_after) {
                    continue;
                }
            }
            if x(at) - from > limit {
                return if last > start { last } else { at };
            }
            last = at;
        }
        // Хвост за последней разрешённой точкой не влез — разрыв по ней.
        // Прежде возвращался весь отрезок: ряд «XX<span>XX</span>XXX» с
        // `overflow-wrap: anywhere` на `<span>` рвался внутри него, но
        // последняя точка (между `<span>` и хвостом) терялась, и хвост
        // уезжал за край вместе с частью `<span>`.
        if last > start && x(end) - from > limit {
            return last;
        }
        end
    }
}

impl Paragraph {
    /// Есть ли между `start` и `end` кусок с разрешённым аварийным разрывом
    /// (см. `emergency_ok`).
    pub(crate) fn emergency_inside(&self, start: usize, end: usize) -> bool {
        self.spans.iter().any(|(r, w)| {
            r.start < end
                && r.end > start
                && !w.nowrap
                && (w.break_all || w.anywhere || w.break_word || w.wrap_anywhere)
        })
    }
}

impl Paragraph {
    /// Конец измеряемой части строки: висящий хвост срезается ПО МЕСТУ.
    ///
    /// Пробел куска с `break-spaces` (и `pre`) не висит (css-text-3 §4.1.3:
    /// «treated the same as other visible characters»), а висеть может только
    /// то, что стоит у самого края, — значит, и всё ПЕРЕД ним остаётся в
    /// строке (`hanging-whitespace-001`: U+3000 абзаца `normal` перед
    /// `<span style="white-space:break-spaces"> </span>`). `spaces_are_content`
    /// смотрит правило абзаца и вложенного куска не видит. Без таких кусков
    /// результат совпадает с `trim_hanging`.
    pub(crate) fn hang_tail(&self, start: usize, end: usize) -> usize {
        // A ruby base/annotation unit is laid out by its own sub-line breaker
        // (Blink line_breaker.cc: ruby columns), whose trailing spaces do not
        // hang: `<ruby>　　あ　　<rt>…</ruby>` keeps its 5em base
        // (`ruby-overhang-spaces-*-ref`).
        if self.ruby_unit {
            return end;
        }
        let mut at = end;
        for (i, ch) in self.text[start..end].char_indices().rev() {
            if ch == '\u{feff}' || !(hangs(ch) || zero_width(ch)) {
                break;
            }
            if ch != '\n' && hangs(ch) {
                let w = self.wrap_at(start + i);
                if w.break_spaces || (w.keep_spaces && w.nowrap) {
                    break;
                }
            }
            at = start + i;
        }
        at
    }
}
