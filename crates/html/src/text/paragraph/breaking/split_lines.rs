//! Вход резчика строк: `split` с памятью разрезов по ключу в кэше документа (разрез
//! гоняется по 3–5 раз на абзац за кадр), `split_uncached` — набор строк, усечение
//! `text-overflow: ellipsis` (css-overflow-3), обрыв `line-clamp` и подбор
//! самой узкой колонки при `text-wrap: balance` вместе с обрывом (место под
//! знак обрыва входит в колонку, css-text-4 §5). Здесь же выравнивание длин
//! строк по группам между жёсткими разрывами (css-text-4 §7.1) и `lay`.

use crate::text::paragraph::*;
use gpui::{Pixels, Window, px};

impl Paragraph {
    /// Разбить текст на строки под заданную ширину и оборвать по `line-clamp`.
    ///
    /// Порядок важен: сперва обрыв, потом выравнивание длин. Выровнять надо
    /// то, что ОСТАЛОСЬ видимым, и с учётом места, отнятого многоточием
    /// (`text-wrap-balance-line-clamp-003`).
    /// Разрез с памятью: раскладка гоняет его по 3-5 раз на абзац за кадр
    /// (min/max/definite у гибкого родителя + подготовка), а разрез — самое
    /// дорогое место резчика. Ключ обязан покрывать ВСЁ, что читает
    /// `split_uncached`, иначе устаревшие переносы сдвинут пиксели.
    pub(crate) fn split(&self, limit: Option<Pixels>, window: &mut Window) -> Vec<Line> {
        self.prepare_hyphen_widths(window);
        // Подбор кегля мутирует абзац между вызовами — ключ это видит
        // (font_size в ключе замера).
        let key = self.split_key(limit);
        if let Some(hit) = self.measures.splits.borrow().get(&key).cloned() {
            return (*hit).clone();
        }
        let lines = self.split_uncached(limit, window);
        {
            let mut m = self.measures.splits.borrow_mut();
            // Прямолинейный сброс при переполнении: страница с тысячами
            // абзацев дороже промахов одного сброса.
            if m.len() >= 2048 {
                m.clear();
            }
            m.insert(key, std::rc::Rc::new(lines.clone()));
        }
        lines
    }
}

impl Paragraph {
    pub(crate) fn split_uncached(&self, limit: Option<Pixels>, window: &mut Window) -> Vec<Line> {
        let segs = self.measure(window);
        let mut lines = self.lay(limit, &segs);
        // Обрезка строк контейнером с `text-overflow: ellipsis`: не влезшая
        // строка усекается с многоточием (css-overflow-3 §text-overflow).
        if self.text_overflow
            && let Some(limit) = limit
        {
            // Строка, на которую сядет знак обрыва `line-clamp`, усекается
            // им самим (css-overflow-4 §block-ellipsis: место отбирается «as
            // if wrapping» до точки переноса, а не посимвольно): непереносимое
            // слово уходит целиком, и остаётся одно «…»
            // (`webkit-line-clamp-036`, `line-clamp-auto-009`).
            let clamp_line = self
                .clamp
                .filter(|n| *n > 0 && (lines.len() > *n || self.clamp_force))
                .map(|n| n.min(lines.len()).saturating_sub(1));
            for (i, line) in lines.iter_mut().enumerate() {
                if Some(i) == clamp_line {
                    continue;
                }
                // CSS Overflow 3: truncate at the content edge; indentation
                // changes the space available from the start of this line.
                let room = limit - line.indent;
                if line.width > room + px(0.5) && !line.ellipsis {
                    self.ellipsize(line, room, &segs, window);
                }
            }
        }
        let cut = self
            .clamp
            .filter(|n| *n > 0 && lines.len() > *n)
            .zip(limit)
            .filter(|_| self.wrap.balance);
        let Some((max, limit)) = cut else {
            return self.clamp_lines(self.balanced(lines, limit, &segs), limit, window);
        };
        // Видимый текст — тот, что уместился в обрезанные строки. Его и
        // раскладываем заново, ища самую узкую колонку, в которой он всё ещё
        // помещается в те же строки.
        let end = self
            .clamp_lines(lines, Some(limit), window)
            .last()
            .map(|l| l.range.end)
            .unwrap_or(0);
        // Место под МНОГОТОЧИЕ входит в колонку: на оборванной строке за
        // текстом рисуется знак обрыва, и колонка, в которую он не влезает,
        // подбором не годится (css-text-4 §5: обрыв — часть последней
        // строки). Ширина берётся без висящего хвоста (`trim_hanging`) — так,
        // как строку оставит сам обрыв (`ellipsize`).
        // clamp_lines may leave an empty final line at the next word's
        // start. Collapsed trailing spaces are not part of the visible text
        // that a narrower layout must retain.
        let end = self.drop_collapsible_tail(0, end);
        let ell = self.suffix_width(self.clamp_str(), 0, window);
        let (mut narrow, mut wide) = (px(0.), limit);
        for _ in 0..12 {
            let middle = (narrow + wide) / 2.;
            let probe = self.lay(Some(middle), &segs);
            let fits = probe.get(max - 1).is_some_and(|l: &Line| {
                if l.range.end < end {
                    return false;
                }
                let cut = l.range.start + trim_hanging(&self.text[l.range.clone()]);
                self.span(&segs, l.range.start, cut) + ell <= middle
            });
            if fits {
                wide = middle;
            } else {
                narrow = middle;
            }
        }
        self.clamp_lines(self.lay(Some(wide), &segs), Some(limit), window)
    }
}

impl Paragraph {
    /// `text-wrap: balance` — те же строки, но одной длины.
    pub(crate) fn balanced(
        &self,
        lines: Vec<Line>,
        limit: Option<Pixels>,
        segs: &[Seg],
    ) -> Vec<Line> {
        let Some(limit) = limit else { return lines };
        if !self.wrap.balance || lines.len() < 2 {
            return lines;
        }
        // Группы строк, разделённые ЖЁСТКИМ разрывом, выравниваются по
        // отдельности (css-text-4 §7.1): у каждой своя ширина, одной на весь
        // абзац не хватает (`text-wrap-balance-004`).
        let mut out: Vec<Line> = Vec::new();
        let mut i = 0usize;
        while i < lines.len() {
            let last = lines[i..]
                .iter()
                .position(|l| self.text[l.range.clone()].ends_with('\n'))
                .map(|k| i + k)
                .unwrap_or(lines.len() - 1);
            let part = lines[i].range.start..lines[last].range.end;
            out.extend(self.balanced_part(part, last + 1 - i, limit, segs));
            i = last + 1;
        }
        out
    }
}

impl Paragraph {
    /// Выравнивание длин ОДНОЙ группы строк: поиск самой узкой колонки, в
    /// которой строк не прибавилось. Тогда последняя строка перестаёт быть
    /// коротким огрызком.
    pub(crate) fn balanced_part(
        &self,
        part: std::ops::Range<usize>,
        target: usize,
        limit: Pixels,
        segs: &[Seg],
    ) -> Vec<Line> {
        if target < 2 {
            return self.lay_in(part, Some(limit), segs);
        }
        // Balance the space left by floats (CSS Text 4 §5.4), rather than
        // buying narrower lines by clearing the float. Clearance is geometry,
        // not an extra line, but must not grow during the width search.
        let clearance = |lines: &[Line]| lines.iter().map(|l| f32::from(l.clearance)).sum::<f32>();
        let max_clearance = clearance(&self.lay_in(part.clone(), Some(limit), segs));
        let (mut narrow, mut wide) = (px(0.), limit);
        for _ in 0..12 {
            let middle = (narrow + wide) / 2.;
            let probe = self.lay_in(part.clone(), Some(middle), segs);
            if probe.len() <= target && clearance(&probe) <= max_clearance {
                wide = middle;
            } else {
                narrow = middle;
            }
        }
        self.lay_in(part, Some(wide), segs)
    }
}

impl Paragraph {
    /// Набор строк под заданную ширину — без выравнивания их длин.
    pub(crate) fn lay(&self, limit: Option<Pixels>, segs: &[Seg]) -> Vec<Line> {
        // CSS 2.1 section 9.4.2: an overlay-only phantom line contributes no
        // height or line count. Its hypothetical static point survives in
        // point_of, which uses the same paragraph's float opportunity.
        if self.text.is_empty() {
            return Vec::new();
        }
        self.lay_in(0..self.text.len(), limit, segs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::float::shapes::FloatShape;
    use std::sync::Arc;
    #[test]
    fn balancing_preserves_the_float_opportunity_instead_of_clearing_it() {
        let mut p = Paragraph::new(
            "aaaa aaaa aaaa".into(),
            vec![],
            px(10.),
            px(20.),
            Align::Left,
            Wrap {
                balance: true,
                ..Default::default()
            },
        );
        p.flow = Arc::new((
            vec![FloatShape::Band {
                top: 0.,
                h: 100.,
                w: 40.,
            }],
            vec![],
        ));
        // Fixed advances keep this geometry test independent of system fonts.
        let segs: Vec<_> = (0..p.text.len())
            .map(|i| Seg {
                start: i,
                end: i + 1,
                offset: px(i as f32 * 10.),
                layout: Arc::new(gpui::LineLayout {
                    width: px(10.),
                    ..Default::default()
                }),
            })
            .collect();
        for (limit, clearance) in [(100., 0.), (60., 100.)] {
            let lines = p.lay(Some(px(limit)), &segs);
            assert_eq!(lines.len(), 3);
            let balanced = p.balanced(lines, Some(px(limit)), &segs);
            assert_eq!(balanced.len(), 3);
            assert_eq!(
                balanced.iter().map(|l| f32::from(l.clearance)).sum::<f32>(),
                clearance
            );
            assert_eq!(
                balanced[0].indent,
                px(if clearance == 0. { 40. } else { 0. })
            );
        }
    }
}
