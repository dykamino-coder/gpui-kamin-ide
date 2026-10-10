//! Wrap policy for breaking; split out to keep the owning module within 250 lines.

use crate::text::paragraph::*;

impl Paragraph {
    /// Неперносима ли точка МЕЖДУ двумя знаками.
    ///
    /// Решает её общий предок (css-text-3 §5.1). У нас предки выражены
    /// диапазонами кусков: если оба знака в ОДНОМ куске, правило его; если в
    /// разных (или один вне кусков) — общий предок это сам абзац.
    pub(crate) fn nowrap_between(&self, at: usize) -> bool {
        let which = |i: usize| self.spans.iter().position(|(r, _)| r.contains(&i));
        let left = which(at.saturating_sub(1));
        let right = which(at);
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.10): точку переноса после ПРОБЕЛА решает
        // `white-space` элемента с самим пробелом (css-text-3 §5.1, «for soft
        // wrap opportunities created by characters that disappear or are
        // preserved spaces»), а не общий предок. Срез 1973 пар (css-text,
        // CSS2/text, <pre>): +1/−3 — white-space-wrap-after-nowrap-001
        // 0.62 -> 0.28, но white-space-007 0.04 -> 11.99,
        // white-space-collapsing-breaks-001 0.00 -> «красное видно». Причина
        // не разобрана; подозрение — после схлопывания через границу куска
        // (`collapse_across_pieces`) уцелевший пробел лежит не в том куске,
        // что у браузера.
        match (left, right) {
            (Some(a), Some(b)) if a == b => self.spans[a].1.nowrap,
            _ => self.wrap.nowrap,
        }
    }
}

impl Paragraph {
    /// Конец строки после шага 3 Phase II (css-text-3 §4.1.3): «A sequence of
    /// collapsible spaces at the end of a line … is removed». Удаляется из
    /// СТРОКИ, а не только из её ширины: подложка куска больше не тянется по
    /// пробелу (`line-break-anywhere-and-white-space-004`). Схлопываемые —
    /// U+0020, табуляция и U+1680 при normal/nowrap/pre-line (§4.1.3); U+3000,
    /// U+00A0 и прочие Zs не схлопываются, они ВИСЯТ и рисуются (★ откат у
    /// `trim_hanging`: обрезка подложки по всему `hangs` ломала
    /// `trailing-ideographic-space-*`).
    pub(crate) fn drop_collapsible_tail(&self, start: usize, end: usize) -> usize {
        let mut at = end;
        for (i, ch) in self.text[start..end].char_indices().rev() {
            if matches!(ch, ' ' | '\t' | '\u{1680}') && !self.wrap_at(start + i).keep_spaces {
                at = start + i;
            } else {
                break;
            }
        }
        at
    }
}

impl Paragraph {
    /// Рвётся ли на этом месте что угодно и где угодно — без оглядки на
    /// типографику (`line-break: anywhere`, `overflow-wrap: anywhere`,
    /// `word-wrap: break-word`).
    pub(crate) fn loose_at(&self, at: usize) -> bool {
        let w = self.wrap_at(at);
        w.anywhere || w.break_word || w.wrap_anywhere
    }
}

impl Paragraph {
    /// Разрешён ли на этом месте аварийный разрыв по знакам.
    pub(crate) fn emergency_ok(&self, at: usize) -> bool {
        let w = self.wrap_at(at);
        !w.nowrap && (w.break_all || w.anywhere || w.break_word || w.wrap_anywhere)
    }
}

impl Paragraph {
    /// Точки, где строку РАЗРЕШЕНО разорвать.
    /// Правила переноса, действующие на байте `at`: сначала свой кусок, потом
    /// абзац целиком.
    /// Сохранённые пробелы конца строки — СОДЕРЖИМОЕ, а не висящие: при
    /// `break-spaces` (они занимают место и дают разрыв) и при `pre`
    /// (css-text-3 §4.1.3 висят только `normal`/`nowrap`/`pre-line` — без
    /// условий — и `pre-wrap` — условно; `pre` в списке нет:
    /// `white-space-intrinsic-size-015`, эталон `eol-spaces-bidi-004`).
    pub(crate) fn spaces_are_content(&self) -> bool {
        self.wrap.break_spaces || (self.wrap.keep_spaces && self.wrap.nowrap)
    }
}

impl Paragraph {
    pub(crate) fn wrap_at(&self, at: usize) -> Wrap {
        self.spans
            .iter()
            .find(|(r, _)| r.contains(&at))
            .map(|(_, w)| *w)
            .unwrap_or(self.wrap)
    }
}
