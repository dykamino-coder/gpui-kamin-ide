//! Shape visible line suffixes while retaining the source soft hyphen style.

use super::*;

impl Paragraph {
    pub(super) fn prepare_hyphen_widths(&self, window: &mut Window) {
        if self.hyphen.is_empty() || !self.hyphen_w.borrow().is_empty() {
            return;
        }
        // Resolve the advance from each conditional hyphen's run, before
        // consulting cached line breaks. Different inline styles can give
        // different advances within one paragraph (CSS Text 3 §5.4).
        let widths = self
            .text
            .char_indices()
            .filter_map(|(at, ch)| {
                (ch == SOFT_HYPHEN).then(|| {
                    (
                        at + ch.len_utf8(),
                        self.suffix_width(&self.hyphen, at, window),
                    )
                })
            })
            .collect();
        *self.hyphen_w.borrow_mut() = widths;
    }

    pub(super) fn hyphen_width(&self, end: usize) -> Pixels {
        self.hyphen_w
            .borrow()
            .iter()
            .find(|(at, _)| *at == end)
            .map_or(px(0.), |(_, width)| *width)
    }

    /// Набрать кусок текста; правый прогон — со знаком стороны письма.
    pub(super) fn shape(
        &self,
        range: &std::ops::Range<usize>,
        runs: &[TextRun],
        rtl: bool,
        suffix: &str,
        window: &mut Window,
    ) -> Option<gpui::ShapedLine> {
        let mut piece = slice_runs(runs, range);
        if piece.is_empty() {
            return None;
        }
        // CSS Text 3 §5.4: an inserted hyphen inherits the soft hyphen's
        // style even when that otherwise invisible run has no painted glyphs.
        let hyphen_run = (suffix == self.hyphen.as_ref()
            && self.text[..range.end].ends_with(SOFT_HYPHEN))
        .then(|| slice_runs(runs, &(range.end - SOFT_HYPHEN.len_utf8()..range.end)))
        .and_then(|runs| runs.into_iter().next());
        // Управляющие знаки не рисуются: своей ширины у них нет, но подмена
        // шрифта может подставить вместо них пустой квадрат и раздвинуть
        // строку. Разрывы по ним УЖЕ решены — здесь остаётся только показ.
        //
        // ПРОБОВАЛИ И ОТКАТИЛИ: не выбрасывать их, чтобы замер и показ считали
        // один и тот же текст (замер берёт его целиком). Счёт не изменился,
        // а `trim_runs` и `invisible` становились мёртвым кодом.
        let body: String = self.text[range.clone()]
            .chars()
            .filter(|c| !invisible(*c))
            .collect();
        if body.len() != range.len() {
            piece = trim_runs(&piece, &self.text[range.clone()]);
        }
        // Знак переноса набирается ВМЕСТЕ со строкой, а не отдельным вызовом:
        // отдельный набор садится на свою базовую линию и сдвигает строку
        // (`hyphens-manual-011`: текст уезжал на три точки вниз).
        let body = if suffix.is_empty() {
            body
        } else {
            // Знак обрыва — свой прогон в стиле блока; знак переноса —
            // часть слова и идёт стилем своего куска.
            if let Some(mut run) = hyphen_run {
                run.len = suffix.len();
                piece.push(run);
            } else if self.is_block_mark(suffix) && self.marker_font.is_some() {
                if let Some(last) = piece.last() {
                    let mut run = last.clone();
                    run.len = suffix.len();
                    self.style_marker_run(suffix, &mut run);
                    piece.push(run);
                }
            } else if let Some(last) = piece.last_mut() {
                last.len += suffix.len();
            }
            format!("{body}{suffix}")
        };
        let body = body.as_str();
        let body = controlled_shape::text(body, &mut piece, rtl);
        if !rtl {
            return Some(window.text_system().shape_line_spaced(
                body,
                self.font_size,
                &piece,
                None,
                self.letter_spacing,
            ));
        }
        Some(
            window
                .text_system()
                .shape_line_rtl(body, self.font_size, &piece, self.letter_spacing),
        )
    }
}
