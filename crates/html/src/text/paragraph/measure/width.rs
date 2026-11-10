//! Замер абзаца (`measure`, с памятью документа между кадрами) и положения знаков:
//! текст меряется по кускам между переводами строки, табуляциями и
//! мягкими переносами (`Seg` со смещением от позиции табуляции), ширина
//! отрезка `span` и положение байта `x_at` с толкованием границы по её роли
//! (конец прошлого куска / начало нового) и добавками трекинга кусков и
//! `word-spacing`, о которых набор не знает.

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
        let hit = self.measures.measured.borrow_mut().get(&key).cloned();
        crate::work::cache!(hit, MeasureCacheHits, MeasureCacheMisses);
        if let Some(segs) = hit {
            self.keep_chunks(key);
            return segs;
        }
        let (out, chunks) = self.measure_uncached(window);
        self.remember_chunks(key, chunks);
        self.measures.measured.borrow_mut().insert(key, out.clone());
        out
    }
}

impl Paragraph {
    fn measure_uncached(&self, window: &mut Window) -> (Vec<Seg>, Vec<u64>) {
        let mut out = Vec::new();
        let mut chunks = Vec::new();
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
            // Длинный кусок набирается частями (`chunks`): каждая — свой `Seg`
            // со сдвигом, как кусок за мягким переносом.
            let parts = self.chunk_ends(start, end, window);
            let ends = parts.as_deref().unwrap_or(std::slice::from_ref(&end));
            let (mut from, mut width) = (start, px(0.));
            for &to in ends {
                let layout = if parts.is_some() {
                    self.shape_chunk(from..to, &mut chunks, window)
                } else {
                    self.shape(from..to, window)
                };
                // Ширина вместе с трекингом кусков и `word-spacing`: набор их
                // не знает, `x_at` добавляет их сам — и позиция табуляции за
                // куском обязана их учесть (`word-spacing-characters-001`).
                width = layout.width + self.seg_extra(from, to);
                out.push(Seg {
                    positions: Default::default(),
                    start: from,
                    end: to,
                    layout,
                    offset,
                });
                if to < end {
                    offset += width;
                }
                from = to;
            }
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
        (out, chunks)
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
        if self.joined_spacing() {
            return self.spacing_before(start, end);
        }
        let mut extra = px(0.);
        if self.letter_spans.is_empty() && self.word_spacing == px(0.) && self.word_spans.is_empty()
        {
            return extra;
        }
        for (off, ch) in self.text[start..end].char_indices() {
            let at = start + off;
            if let Some(i) = self.prepared().letters.first(at) {
                extra += self.letter_spans[i].1 - self.letter_spacing;
            }
            if word_separator(ch) {
                extra += self
                    .prepared()
                    .words
                    .first(at)
                    .map_or(self.word_spacing, |i| self.word_spans[i].1);
            }
        }
        extra
    }
}

impl Paragraph {
    pub(crate) fn x_at(&self, segs: &[Seg], i: usize, edge: Edge) -> Pixels {
        crate::work::add!(TextPositionQueries, 1);
        let i = i.min(self.text.len());
        let after_break = edge == Edge::End && i > 0 && self.text.as_bytes()[i - 1] == b'\n';
        // Шаги поиска копятся локально и сдаются одним вызовом: наблюдение
        // на каждом шаге стоило дороже самого поиска.
        #[cfg(feature = "work-counters")]
        let steps = std::cell::Cell::new(0u64);
        let index = segs.partition_point(|s| {
            #[cfg(feature = "work-counters")]
            steps.set(steps.get() + 1);
            if after_break {
                s.end + 1 < i
            } else {
                s.end < i
            }
        });
        crate::work::add!(TextPositionSteps, steps.get());
        let Some(seg) = segs.get(index).or_else(|| segs.last()) else {
            return px(0.);
        };
        let positions = seg.positions.get_or_init(|| self.segment_positions(seg));
        positions[i.clamp(seg.start, seg.end) - seg.start]
    }
}
