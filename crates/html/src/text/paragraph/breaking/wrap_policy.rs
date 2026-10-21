//! Правила переноса ПО МЕСТУ в тексте: какой `Wrap` действует на байте
//! (свой кусок, иначе абзац), неперносима ли точка между двумя знаками по
//! их общему предку (css-text-3 §5.1), разрешён ли аварийный разрыв и
//! разрыв «где угодно», считаются ли сохранённые пробелы содержимым, и
//! удаление схлопываемых пробелов с конца строки (css-text-3 §4.1.3).

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
        // Не решать точку переноса после ЛЮБОГО пробела по `white-space`
        // элемента с самим пробелом вместо общего предка (css-text-3 §5.1,
        // «soft wrap opportunities created by characters that disappear or
        // are preserved spaces»): `white-space-007` и
        // `white-space-collapsing-breaks-001` ломаются — после схлопывания
        // через границу куска (`collapse_across_pieces`) уцелевший пробел
        // лежит не в том куске, что у браузера. Ниже — узкое правило только
        // для сохраняемых пробелов.
        // CSS Text 3 §5.1: preserved spaces own their soft wrap
        // opportunities, including across the closing bidi isolate.
        if let Some(i) = left
            && self.spans[i].1.keep_spaces
            && self.spans[i].1.nowrap
            && matches!(
                self.text[..at]
                    .chars()
                    .rev()
                    .find(|c| !matches!(c, '\u{2069}' | '\u{202c}')),
                Some(' ' | '\t' | '\u{1680}')
            )
        {
            return true;
        }
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
    /// U+00A0 и прочие Zs не схлопываются, они ВИСЯТ и рисуются (обрезать
    /// подложку по всему `hangs` в `trim_hanging` нельзя: ломает
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
        !w.nowrap && (w.anywhere || w.break_word || w.wrap_anywhere)
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
