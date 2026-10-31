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
        if let Some(hit) = hit {
            return hit;
        }
        let out = self.measure_uncached(window);
        self.measures.measured.borrow_mut().insert(key, out.clone());
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
            let runs = self.own_runs(&(start..end));
            let layout = window
                .text_system()
                .with_ligature_breaking(false)
                .layout_line_spaced(
                    &self.text[start..end],
                    self.font_size,
                    &runs,
                    None,
                    if self.joined_spacing() {
                        px(0.)
                    } else {
                        self.letter_spacing
                    },
                );
            // Ширина куска вместе с трекингом кусков и `word-spacing`: набор
            // их не знает, `x_at` добавляет их сам — и позиция табуляции за
            // куском обязана их учесть (`word-spacing-characters-001`:
            // табуляция после растянутых пробелов вставала раньше).
            let width = layout.width + self.seg_extra(start, end);
            out.push(Seg {
                positions: Default::default(),
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
        let i = i.min(self.text.len());
        let after_break = edge == Edge::End && i > 0 && self.text.as_bytes()[i - 1] == b'\n';
        let index = if after_break {
            segs.partition_point(|s| s.end + 1 < i)
        } else {
            segs.partition_point(|s| s.end < i)
        };
        let Some(seg) = segs.get(index).or_else(|| segs.last()) else {
            return px(0.);
        };
        let positions = seg.positions.get_or_init(|| self.segment_positions(seg));
        positions[i.clamp(seg.start, seg.end) - seg.start]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexed_positions_keep_tab_hyphen_and_newline_edge_semantics() {
        let text = "ab\tcd\u{ad}ef\ngh\n";
        for align in [Align::Left, Align::Justify] {
            let p = Paragraph::new(
                text.into(),
                vec![],
                px(16.),
                px(20.),
                align,
                Wrap::default(),
            )
            .letter_spans(vec![(0..text.len(), px(0.1)), (0..2, px(5.))])
            .word_spacing(px(0.3));
            let mut start = 0;
            let mut segs = Vec::new();
            for (end, ch) in text
                .char_indices()
                .filter(|(_, c)| matches!(*c, '\t' | '\n' | SOFT_HYPHEN))
                .chain([(text.len(), '\0')])
            {
                segs.push(Seg {
                    start,
                    end,
                    offset: px(start as f32 * 3.7),
                    positions: Default::default(),
                    layout: std::sync::Arc::new(gpui::LineLayout {
                        len: end - start,
                        width: px((end - start) as f32 * 7.3),
                        ..Default::default()
                    }),
                });
                start = end + ch.len_utf8();
            }
            for i in text.char_indices().map(|(at, _)| at).chain([text.len()]) {
                for edge in [Edge::Start, Edge::End] {
                    let after = edge == Edge::End && i > 0 && text.as_bytes()[i - 1] == b'\n';
                    let seg = segs
                        .iter()
                        .find(|s| if after { s.end + 1 == i } else { i <= s.end })
                        .or_else(|| segs.last())
                        .unwrap();
                    let mut expected = if i >= seg.end {
                        seg.layout.width
                    } else if i <= seg.start {
                        px(0.)
                    } else {
                        seg.layout.x_for_index(i - seg.start)
                    } + seg.offset;
                    for (at, _) in text[seg.start..i.min(seg.end)].char_indices() {
                        if let Some((_, v)) = p
                            .letter_spans
                            .iter()
                            .find(|(r, _)| r.contains(&(seg.start + at)))
                        {
                            expected += *v - p.letter_spacing;
                        }
                    }
                    assert_eq!(p.x_at(&segs, i, edge), expected, "{i}, after={after}");
                }
            }
        }
    }
}
