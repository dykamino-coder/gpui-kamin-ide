//! Overflow config for model; split out to keep the owning module within 250 lines.

use super::Indent;
use super::Paragraph;
use crate::text::paragraph::tabs;
use gpui::{AnyElement, Hsla, Pixels, SharedString};

impl Paragraph {
    /// Расходится ли трекинг ОТРЕЗКОВ с общим трекингом абзаца.
    ///
    /// `letter_spans` заполняется на КАЖДЫЙ кусок с заданным `letter-spacing`,
    /// а свойство наследуется: `p { letter-spacing: 1em }` даёт запись на все
    /// куски с тем же значением, что и общее. Пустой разницы достаточно, чтобы
    /// абзац ушёл на пословную краску, где видимого порядка UAX#9 нет вовсе —
    /// и буквы вставали в логическом порядке (`bidi-005b`…`-009b`).
    ///
    /// Незримый знак (распорка полей, метка атома, управление
    /// двунаправленностью) несёт свой трекинг всегда: у него он и есть
    /// продвижение, поэтому расхождением считается любое НЕнулевое значение.
    pub(crate) fn letter_spans_diverge(&self) -> bool {
        let common = f32::from(self.letter_spacing);
        self.letter_spans.iter().any(|(r, v)| {
            let seen = f32::from(*v);
            let body = self.text.get(r.clone()).unwrap_or("");
            let invisible = !body.is_empty()
                && body.chars().all(|c| {
                    matches!(
                        c,
                        '\u{feff}' | '\u{200b}' | '\u{200e}' | '\u{200f}'
                            | '\u{202a}'..='\u{202e}'
                            | '\u{2066}'..='\u{2069}'
                    )
                });
            // Незримый знак с ОБЩИМ трекингом — просто унаследовавший его
            // знак управления двунаправленностью: расхождением он не является.
            // Расходится только распорка, чей трекинг и есть её ширина.
            if invisible && (seen - common).abs() <= 0.01 {
                return false;
            }
            if invisible {
                seen != 0.0
            } else {
                (seen - common).abs() > 0.01
            }
        })
    }
}

impl Paragraph {
    /// Отступ первой строки (`text-indent`).
    pub fn indent(mut self, indent: Indent) -> Self {
        self.indent = indent;
        self
    }
}

impl Paragraph {
    /// `text-justify` mode (see `justify_chars`).
    pub fn justify_chars(mut self, mode: u8) -> Self {
        self.justify_chars = mode;
        self
    }
}

impl Paragraph {
    /// `line-clamp`: сколько строк оставить.
    pub fn line_clamp(mut self, lines: Option<usize>) -> Self {
        self.clamp = lines;
        self
    }
}

impl Paragraph {
    /// Ставить знак обрыва и тогда, когда абзац влез целиком: точка среза
    /// стоит сразу за ним (авто-режим, css-overflow-4 §5.3).
    pub fn clamp_marked(mut self, on: bool) -> Self {
        self.clamp_force = on;
        self
    }
}

impl Paragraph {
    /// `text-overflow: ellipsis` контейнера: строка, не влезшая в колонку
    /// (nowrap/pre — переносов нет), усекается и получает многоточие.
    pub fn text_ellipsis(mut self, on: bool) -> Self {
        self.text_overflow = on;
        self
    }
}

impl Paragraph {
    /// Маркер обрезки: строка из `text-overflow: <string>`, шрифт И КЕГЛЬ
    /// блока (css-overflow-4 §5 — маркер оформлен как блок).
    pub fn overflow_marker(
        mut self,
        mark: Option<String>,
        font: Option<gpui::Font>,
        size: Option<Pixels>,
    ) -> Self {
        self.overflow_marker = mark;
        self.marker_font = font;
        self.marker_size = size;
        self
    }
}

impl Paragraph {
    /// Метка абзаца в клэмп-контейнере (см. поле `clamp_tag`).
    pub fn clamp_tag(mut self, tag: Option<(u64, u32)>) -> Self {
        self.clamp_tag = tag;
        self
    }
}

impl Paragraph {
    /// `block-ellipsis` контейнера: знак строки обрыва `line-clamp`.
    pub fn clamp_mark(mut self, mark: Option<String>) -> Self {
        self.clamp_marker = mark;
        self
    }
}

impl Paragraph {
    /// Цвет знака обрыва — цвет блока.
    pub fn marker_color(mut self, color: Option<Hsla>) -> Self {
        self.marker_color = color;
        self
    }
}

impl Paragraph {
    /// Чем показывать перенос слова.
    pub fn hyphen_char(mut self, mark: Option<String>) -> Self {
        if let Some(mark) = mark {
            self.hyphen = SharedString::from(mark);
        }
        self
    }
}

impl Paragraph {
    /// Шаг позиций табуляции.
    pub fn tab_stops(mut self, step: tabs::TabStops) -> Self {
        self.tab_stop = step;
        self
    }
}

impl Paragraph {
    /// Куски вне потока: место в тексте → элемент.
    pub fn overlays(
        mut self,
        overlays: Vec<(usize, AnyElement, crate::text::inline::OverlayAt)>,
    ) -> Self {
        self.overlays = overlays;
        self
    }
}

impl Paragraph {
    /// `text-fit`: подбирать ли кегль под ширину коробки.
    pub fn text_fit(mut self, fit: Option<crate::style::computed::TextFit>) -> Self {
        self.fit = fit;
        self
    }
}

impl Paragraph {
    /// Какие части абзаца подбор кегля вправе масштабировать.
    pub fn fit_parts(mut self, spacing_scalable: bool, line_height_fixed: bool) -> Self {
        self.fit_spacing_scalable = spacing_scalable;
        self.fit_line_height_fixed = line_height_fixed;
        self
    }
}
