//! Marker for paint; split out to keep the owning module within 250 lines.

use crate::text::paragraph::*;
use gpui::{TextRun, Window, px};

impl Paragraph {
    /// Строковый маркер несёт шрифт контейнера; многоточие и знак переноса
    /// остаются в шрифте прогона у среза.
    pub(crate) fn style_marker_run(&self, mark: &str, run: &mut TextRun) {
        // Знак обрыва — содержимое САМОГО БЛОКА, а не куска, на котором
        // строка оборвалась: кегль у него блочный (css-overflow-3 §4.1,
        // «the ellipsis is styled as the block»). Прогон брался с места
        // обрыва вместе со своим кеглем, и внутри `<span style="font-size:
        // 1rem">` в блоке с `4rem` многоточие выходило вчетверо уже нужного
        // (`text-wrap-balance-line-clamp-002`: место под него при подборе
        // колонки считалось 8.8 точки вместо 35.2).
        run.font_size = None;
        // Знак обрыва — анонимный строчный ребёнок САМОГО БЛОКА
        // (css-overflow-4 §5.3 block-ellipsis: «wrapped in an anonymous
        // inline whose parent is the block container»): шрифт, кегль и цвет —
        // блочные, рамки и фона куска у среза у него нет. Эталоны:
        // `block-ellipsis-005` (знак за `<span>` 1.5em bold italic — обычный
        // teal блока), `webkit-line-clamp-031` (за жирным — нежирный).
        if self.is_block_mark(mark)
            && let Some(f) = self.marker_font.as_ref()
        {
            run.font = f.clone();
            run.font_size = self.marker_size;
            if let Some(c) = self.marker_color {
                run.color = c;
            }
            run.background_color = None;
            run.background_border = None;
            run.background_pad = Default::default();
            run.background_radius = px(0.);
            return;
        }
        if let (Some(m), Some(f)) = (self.overflow_marker.as_deref(), self.marker_font.as_ref())
            && mark == m
        {
            run.font = f.clone();
            // Кегль СТРОКИ-ЗАМЕНЫ — блочный, а не базовый кегль абзаца:
            // базовый равен `biggest`, и внутри `<span>` крупнее блока
            // маркер выходил втрое шире (`text-overflow-string-*`: под
            // строку резервировалось ~90 точек вместо 20).
            run.font_size = self.marker_size;
        }
    }
}

impl Paragraph {
    /// Набор с знаком обрыва в начале или в конце куска.
    pub(crate) fn shape_with_mark(
        &self,
        range: &std::ops::Range<usize>,
        runs: &[TextRun],
        rtl: bool,
        suffix: &str,
        at_start: bool,
        window: &mut Window,
    ) -> Option<gpui::ShapedLine> {
        if !at_start || suffix.is_empty() {
            return self.shape(range, runs, rtl, suffix, window);
        }
        // Префикс: знак дорисовывается отдельным вызовом слева, а сам кусок
        // набирается без него (вплетение в шейп меняло бы кернинг начала).
        self.shape(range, runs, rtl, "", window)
    }
}

impl Paragraph {
    #[allow(clippy::needless_return)]
    pub(crate) fn paint_empty_line(
        &self,
        range: &std::ops::Range<usize>,
        at: gpui::Point<gpui::Pixels>,
        suffix: &str,
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) -> (Option<gpui::Pixels>, bool) {
        // Пустой отрезок с ХВОСТОМ — это строка обрыва `line-clamp`, у
        // которой под многоточие не осталось места ни для одного слова
        // (`clamp_lines` схлопывает диапазон в `head..head`). Знак обрыва
        // рисуется в цикле по прогонам ниже, поэтому ранний выход уносил
        // и его: коробка занимала высоту, но многоточия не показывала
        // (`text-wrap-balance-line-clamp-004`).
        if !suffix.is_empty() && !self.text.is_empty() {
            let anchor = range.start.min(self.text.len().saturating_sub(1));
            self.paint_suffix(suffix, anchor, at, None, false, window, cx);
        }
        return (None, false);
    }
}
