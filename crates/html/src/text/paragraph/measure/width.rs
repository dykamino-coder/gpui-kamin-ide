//! Width for measure; split out to keep the owning module within 250 lines.

use super::MEASURE_CACHE;
use super::MEASURED;
use crate::text::paragraph::*;
use gpui::{Pixels, Window, px};

impl Paragraph {
    /// Куски между обязательными разрывами: набор не принимает перевод строки,
    /// поэтому мерить приходится по кускам, а положения знаков сшивать.
    ///
    /// Результат запоминается: раскладка спрашивает размер абзаца по многу раз
    /// за кадр (перебор ширин в гибком контейнере), а набор строки — самая
    /// дорогая операция здесь.
    pub(crate) fn measure(&self, window: &mut Window) -> Vec<Seg> {
        let key = self.measure_key();
        if let Some(hit) = MEASURED.with(|c| {
            c.borrow()
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| v.clone())
        }) {
            return hit;
        }
        let out = self.measure_uncached(window);
        MEASURED.with(|c| {
            let mut cache = c.borrow_mut();
            if cache.len() >= MEASURE_CACHE {
                cache.remove(0);
            }
            cache.push((key, out.clone()));
        });
        out
    }
}

impl Paragraph {
    pub(crate) fn measure_uncached(&self, window: &mut Window) -> Vec<Seg> {
        let mut out = Vec::new();
        let mut start = 0usize;
        let mut offset = px(0.);
        loop {
            // Кусок кончается переводом строки ИЛИ табуляцией: табуляция — не
            // знак со своей шириной, а прыжок к следующей позиции табуляции, и
            // отдавать её набору нечего (`break-spaces-tab`).
            let end = self.text[start..]
                .find(['\n', '\t', SOFT_HYPHEN])
                .map(|i| start + i)
                .unwrap_or(self.text.len());
            let runs = slice_runs(&self.runs, &(start..end));
            let layout = window
                .text_system()
                .with_ligature_breaking(false)
                .layout_line_spaced(
                    &self.text[start..end],
                    self.font_size,
                    &runs,
                    None,
                    self.letter_spacing,
                );
            // Ширина куска вместе с трекингом кусков и `word-spacing`: набор
            // их не знает, `x_at` добавляет их сам — и позиция табуляции за
            // куском обязана их учесть (`word-spacing-characters-001`:
            // табуляция после растянутых пробелов вставала раньше).
            let width = layout.width + self.seg_extra(start, end);
            out.push(Seg {
                start,
                end,
                layout,
                offset,
            });
            if end >= self.text.len() {
                break;
            }
            let mark = self.text[end..].chars().next().unwrap_or('\n');
            match mark {
                '\n' => offset = px(0.),
                // Мягкий перенос своей ширины не имеет: он лишь ПОЗВОЛЯЕТ
                // разрыв. Пока он доезжал до набора, шрифт давал ему ширину
                // дефиса, и слово рвалось раньше времени (`hyphens-manual-011`:
                // «Deoxy-ribo-» вместо «Deoxyribo-»).
                SOFT_HYPHEN => offset += width,
                _ => {
                    let x = f32::from(offset + width);
                    let next = self.tab_stop.next(end, x);
                    offset = px(next);
                }
            }
            start = end + mark.len_utf8();
        }
        out
    }
}

impl Paragraph {
    /// Ширина отрезка строки.
    ///
    /// Положения знаков считаются от начала своего куска, поэтому границу
    /// надо толковать по её роли: КОНЕЦ отрезка сразу за переводом строки —
    /// это конец прошлого куска, а НАЧАЛО с тем же индексом — начало нового.
    /// Иначе строка, открывающая новый кусок, получала ширину со знаком минус
    /// и уезжала за край коробки.
    pub(crate) fn span(&self, segs: &[Seg], from: usize, to: usize) -> Pixels {
        if to <= from {
            return px(0.);
        }
        let width = self.x_at(segs, to, Edge::End) - self.x_at(segs, from, Edge::Start);
        if width < px(0.) { px(0.) } else { width }
    }
}

impl Paragraph {
    /// Положение знака от начала своего куска.
    /// Добавка к ширине набранного куска `start..end`: трекинг кусков сверх
    /// общего и `word-spacing` у пробелов (то же, что `x_at` прибавляет к
    /// положению знака в конце куска).
    pub(crate) fn seg_extra(&self, start: usize, end: usize) -> Pixels {
        let mut extra = px(0.);
        if self.letter_spans.is_empty() && self.word_spacing == px(0.) && self.word_spans.is_empty()
        {
            return extra;
        }
        for (off, ch) in self.text[start..end].char_indices() {
            let at = start + off;
            if let Some((_, v)) = self.letter_spans.iter().find(|(r, _)| r.contains(&at)) {
                extra += *v - self.letter_spacing;
            }
            if word_separator(ch) {
                extra += self
                    .word_spans
                    .iter()
                    .find(|(r, _)| r.contains(&at))
                    .map(|(_, v)| *v)
                    .unwrap_or(self.word_spacing);
            }
        }
        extra
    }
}

impl Paragraph {
    pub(crate) fn x_at(&self, segs: &[Seg], i: usize, edge: Edge) -> Pixels {
        let i = i.min(self.text.len());
        let after_break = edge == Edge::End && i > 0 && self.text.as_bytes()[i - 1] == b'\n';
        let seg = if after_break {
            segs.iter().find(|s| s.end + 1 == i)
        } else {
            segs.iter().find(|s| i <= s.end)
        };
        let Some(seg) = seg.or_else(|| segs.last()) else {
            return px(0.);
        };
        let base = if i >= seg.end {
            seg.layout.width
        } else if i <= seg.start {
            px(0.)
        } else {
            seg.layout.x_for_index(i - seg.start)
        };
        // Сдвиг куска внутри строки: его задаёт табуляция перед ним.
        let mut base = base + seg.offset;
        if !self.letter_spans.is_empty() {
            let upto = i.min(seg.end);
            for (off, _) in self.text[seg.start..upto].char_indices() {
                let at = seg.start + off;
                if let Some((_, v)) = self.letter_spans.iter().find(|(r, _)| r.contains(&at)) {
                    base += *v - self.letter_spacing;
                }
            }
        }
        if self.word_spacing == px(0.) && self.word_spans.is_empty() {
            return base;
        }
        // Набор про `word-spacing` не знает: знак сдвинут на столько добавок,
        // сколько пробелов осталось позади него внутри куска. Добавка у
        // каждого пробела СВОЯ — заданная на том куске, в который он попал.
        let upto = i.min(seg.end);
        let mut extra = px(0.);
        for (off, _) in self.text[seg.start..upto]
            .char_indices()
            .filter(|(_, c)| word_separator(*c))
        {
            let at = seg.start + off;
            extra += self
                .word_spans
                .iter()
                .find(|(r, _)| r.contains(&at))
                .map(|(_, v)| *v)
                .unwrap_or(self.word_spacing);
        }
        base + extra
    }
}
