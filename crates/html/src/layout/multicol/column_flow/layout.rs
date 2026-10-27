//! Замер рядов текстовых колонок и экспорт базовых линий.

use super::element::ColumnFlow;
use super::{intrinsic::intrinsic_column_width, measure::measure_columns};
use gpui::{AvailableSpace, LayoutId, Style, Window, px, size};

impl ColumnFlow {
    pub(super) fn measure_layout(&self, window: &mut Window) -> LayoutId {
        let text = self.text.clone();
        let count = self.count;
        let col_w = self.col_w;
        let gap = self.gap;
        let font = self.font.clone();
        let font_size = self.font_size;
        let line_height = self.line_height;
        let cuts = self.cuts.clone();
        let fill_height = self.fill_height;
        let whole = self.whole;
        let line_breaks = self.line_breaks;
        let outer = self.outer.clone();
        let rows = self.rows.clone();
        // css-multicol-1 §Overflow: заданная блочная высота ограничивает высоту
        // КОЛОНКИ, а не всей стопки — с ней рождаются переполняющие колонки.
        window.request_measured_layout_with_baselines(
            Style::default(),
            move |known, available, window, _cx| {
                // Под `min-content`/`max-content` доступного места НЕТ, и
                // «ширина окна» здесь была выдумкой: `width: min-content` у
                // многоколоночника не значил ничего, и коробка растягивалась
                // на весь кадр (`multicol-width-004/005` — все четыре
                // `<article>` во всю ширину). Считаем внутренний размер по
                // css-sizing-4 §multicol-intrinsic, как Blink
                // `column_layout_algorithm.cc:433`.
                let width = known.width.unwrap_or(match available.width {
                    AvailableSpace::Definite(w) => w,
                    other => intrinsic_column_width(
                        &text,
                        count,
                        col_w,
                        gap,
                        &font,
                        font_size,
                        matches!(other, AvailableSpace::MinContent),
                        window,
                    ),
                });
                let (at, used, height, first_col_lines) = measure_columns(
                    &text,
                    count,
                    col_w,
                    gap,
                    &font,
                    font_size,
                    line_height,
                    fill_height,
                    whole,
                    line_breaks,
                    width,
                    outer.as_deref(),
                    &mut rows.borrow_mut(),
                    window,
                )
                .expect("resolved outer fragmentainer sequence");
                *cuts.borrow_mut() = (at, width, used);
                // Базовые линии css-align-3 учитывают смещения рядов и нижнюю
                // строку содержимого; свободный остаток ряда в них не входит.
                let id = window.text_system().resolve_font(&font);
                let ascent = window.text_system().ascent(id, px(font_size));
                let descent = window.text_system().descent(id, px(font_size));
                let first = (px(line_height) - (ascent + descent.abs())) / 2.0 + ascent;
                let rows = rows.borrow();
                let last = first
                    + px(rows.last().map_or(
                        line_height * first_col_lines.saturating_sub(1) as f32,
                        |r| r.y + line_height * r.lines.saturating_sub(1) as f32,
                    ));
                let first = first + px(rows.first().map_or(0., |r| r.y));
                (size(width, height), Some(first), Some(last))
            },
        )
    }
}
