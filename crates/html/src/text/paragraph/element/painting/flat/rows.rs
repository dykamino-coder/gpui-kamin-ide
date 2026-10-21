//! Вспомогательные шаги отрисовки абзаца: публикация рядов строк для
//! `ClampCut` (`publish_rows` в `ParagraphRows` документа), прогоны с учётом выделения (без
//! подчёркиваний GPUI, когда оформление рисует сам абзац), выключенная
//! строка со знаком обрыва на её базовой линии, и смещение первой строки
//! при обратном порядке строк (`vertical-lr`) одним сложением от верха
//! коробки, как у `point_of`.

use crate::text::paragraph::*;
use gpui::{Bounds, Pixels, point, px};

impl Paragraph {
    pub(crate) fn publish_rows(
        &self,
        bounds: Bounds<Pixels>,
        count: usize,
        step: &dyn Fn(usize) -> Pixels,
    ) {
        if let Some(tag) = self.clamp_tag
            && !self.lines_reversed
        {
            let mut y0 = f32::from(bounds.origin.y);
            let rows = match &self.unbalanced_steps {
                Some(steps) => steps
                    .iter()
                    .map(|h| {
                        let r = (y0, y0 + h);
                        y0 = r.1;
                        r
                    })
                    .collect(),
                None => (0..count)
                    .map(|i| {
                        let r = (y0, y0 + f32::from(step(i)));
                        y0 = r.1;
                        r
                    })
                    .collect(),
            };
            self.clamp_rows.publish(tag, rows);
        }
    }
}

impl Paragraph {
    pub(crate) fn selection_runs(
        &self,
        id: Option<&gpui::GlobalElementId>,
        window: &mut gpui::Window,
    ) -> Vec<gpui::TextRun> {
        let selection = id
            .map(|global| {
                window.with_element_state::<Selection, _>(global, |state, _| {
                    let st = state.unwrap_or_default();
                    (st.range(), st)
                })
            })
            .unwrap_or((0, 0));
        let mut runs = self.runs_with_selection(selection.0, selection.1);
        if self.decor_on() {
            for r in runs.iter_mut() {
                r.underline = None;
                r.strikethrough = None;
            }
        }
        runs
    }
}

impl Paragraph {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn paint_justified_line(
        &self,
        line: &Line,
        range: &std::ops::Range<usize>,
        segs: &[Seg],
        free: gpui::Pixels,
        bounds: gpui::Bounds<gpui::Pixels>,
        y: gpui::Pixels,
        pad: (f32, f32),
        dx: gpui::Pixels,
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) {
        self.paint_justified(
            range, segs, free, line.width, bounds, y, pad, dx, window, cx,
        );
        if line.ellipsis {
            let text = self.span(segs, line.range.start, range.end);
            let (_, _, step, absorbed, _) = self.prepare_words(range, segs, free);
            let expanded =
                step * self.justify_opps(range, range.end).saturating_sub(absorbed) as f32;
            self.paint_suffix(
                self.line_mark(line),
                line.range.start,
                point(bounds.origin.x + dx + text + expanded, y),
                // На базовую линию строки — ту же, на которую
                // `paint_justified` опускает слова (`block-ellipsis-005`:
                // знак кегля блока за `<span>` 1.5em стоял выше).
                self.base_of(range).map(px),
                false,
                window,
                cx,
            );
        }
    }
}

impl Paragraph {
    pub(crate) fn reversed_offset(&self, count: usize, step: &dyn Fn(usize) -> Pixels) -> f32 {
        // Строки снизу вверх: место строки считается ОТ ВЕРХА коробки одним
        // сложением (`origin + px(смещение)`), как у `point_of`. Прежде
        // `origin + total - line_height` в f32 расходился с `point_of` в
        // последнем знаке, а глиф (`paint_glyph`: `floor` физической точки)
        // на ровной точке от этого падает на целую точку: текст стоял на
        // точку от щупа статической позиции — столбец красного в
        // `static-position/vlr-*` (замер по снимку: глиф x=49, коробка 48;
        // после правки оба 48).
        let rev_off: f32 = if self.lines_reversed && count > 0 {
            let total: f32 = (0..count).map(|i| f32::from(step(i))).sum();
            // The first reversed line owns its entire advance, including
            // annotation outsets. Its above inset is applied when painted.
            total - f32::from(step(0))
        } else {
            0.0
        };
        rev_off
    }
}
