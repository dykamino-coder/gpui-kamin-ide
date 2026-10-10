//! Configuration for model; split out to keep the owning module within 250 lines.

use super::Paragraph;
use super::{Align, Wrap};
use crate::text::paragraph::probes::*;
use gpui::{ElementId, Hsla, Pixels};

impl Paragraph {
    /// Сдвиг кусков по вертикали: отрезок байт → смещение базовой линии.
    pub fn rel_spans(mut self, spans: Vec<(std::ops::Range<usize>, (f32, f32))>) -> Self {
        self.rel_spans = spans;
        self
    }
}

impl Paragraph {
    pub fn lh_spans(mut self, spans: Vec<(std::ops::Range<usize>, Pixels)>) -> Self {
        self.lh_spans = spans;
        self
    }
}

impl Paragraph {
    /// Строчные коробки кусков и шрифт струта (см. поле `box_spans`).
    pub fn line_boxes(
        mut self,
        spans: Vec<(std::ops::Range<usize>, f32)>,
        strut: Option<(gpui::Font, Pixels)>,
    ) -> Self {
        self.box_spans = spans;
        self.strut_run = strut;
        self
    }
}

impl Paragraph {
    /// Куски с украшениями (см. поле `decor_spans`).
    pub fn decor_spans(mut self, spans: Vec<DecorSpan>) -> Self {
        self.decor_spans = spans;
        self
    }
}

impl Paragraph {
    /// Куски со знаком акцента (см. поле `emph_spans`).
    pub fn emph_spans(mut self, spans: Vec<EmphSpan>) -> Self {
        self.emph_spans = spans;
        self
    }
}

impl Paragraph {
    /// `text-box-trim` блока (см. поле `ruby_trim`).
    pub fn ruby_trim(mut self, start: bool, end: bool) -> Self {
        self.ruby_trim = (start, end);
        self
    }
}

impl Paragraph {
    /// Куски, прижатые к краю строки (см. поле `edge_spans`).
    pub fn edge_spans(mut self, spans: Vec<(std::ops::Range<usize>, bool, f32)>) -> Self {
        self.edge_spans = spans;
        self
    }
}

impl Paragraph {
    pub fn shift_spans(mut self, spans: Vec<(std::ops::Range<usize>, Pixels)>) -> Self {
        self.shift_spans = spans;
        self
    }
}

impl Paragraph {
    /// Трекинг по кускам: отрезок байт → своё значение.
    pub fn letter_spans(mut self, spans: Vec<(std::ops::Range<usize>, Pixels)>) -> Self {
        self.letter_spans = spans;
        self
    }
}

impl Paragraph {
    /// Межсловный интервал по кускам: отрезок байт → своя добавка.
    pub fn word_spans(mut self, spans: Vec<(std::ops::Range<usize>, Pixels)>) -> Self {
        self.word_spans = spans;
        self
    }
}

impl Paragraph {
    /// Правила переноса по кускам: отрезок байт → своё правило.
    pub fn spans(mut self, spans: Vec<(std::ops::Range<usize>, Wrap)>) -> Self {
        self.spans = spans;
        self
    }
}

impl Paragraph {
    /// Выключка последней строки — своя, если разметка её задала.
    /// `unicode-bidi: plaintext`: логическая выключка, которую надо решать по
    /// стороне КАЖДОЙ строки.
    pub fn plaintext(mut self, align: Option<crate::style::computed::TextAlign>) -> Self {
        self.plaintext = align;
        self
    }
}

impl Paragraph {
    /// Рисовать строки снизу вверх (см. поле `lines_reversed`).
    pub fn reversed_lines(mut self, on: bool) -> Self {
        self.lines_reversed = on;
        self
    }
}

impl Paragraph {
    pub fn align_last(mut self, align: Option<Align>) -> Self {
        self.align_last = align;
        self
    }
}

impl Paragraph {
    /// Трекинг: добавка к каждому знаку (`letter-spacing`).
    pub fn letter_spacing(mut self, extra: Pixels) -> Self {
        self.letter_spacing = extra;
        self
    }
}

impl Paragraph {
    pub fn word_spacing(mut self, extra: Pixels) -> Self {
        self.word_spacing = extra;
        self
    }
}

impl Paragraph {
    /// Предел ортогонального потока (см. поле `ortho_limit`).
    pub fn ortho_limit(mut self, limit: Option<Pixels>) -> Self {
        self.ortho_limit = limit;
        self
    }
}

impl Paragraph {
    /// Вертикальное письмо и сторона набегания строк.
    pub fn vertical(mut self, on: bool, rl: bool) -> Self {
        self.vertical = on;
        self.vertical_rl = rl;
        self
    }
}

impl Paragraph {
    /// Разрешить выделение мышью: абзац заводит своё состояние и область
    /// попадания.
    pub fn selectable(mut self, id: ElementId, highlight: Hsla) -> Self {
        self.id = Some(id);
        self.highlight = highlight;
        self
    }
}

impl Paragraph {
    /// Свисающая пунктуация (`hanging-punctuation`).
    pub fn hanging(mut self, hanging: Option<crate::style::computed::Hanging>) -> Self {
        self.hanging = hanging.unwrap_or_default();
        self
    }
}

impl Paragraph {
    /// Места знаков-распорок строчных коробок.
    pub fn spacers(mut self, spacers: Vec<usize>) -> Self {
        self.spacers = spacers;
        self
    }
}

impl Paragraph {
    /// Edge spacers with their inline boxes.
    pub fn spacer_edges(mut self, edges: Vec<(usize, u32, bool, bool)>) -> Self {
        self.spacer_edges = edges;
        self
    }
}

impl Paragraph {
    /// Content extents of inline boxes with edge spacers.
    pub fn box_extents(mut self, extents: Vec<(u32, usize, usize)>) -> Self {
        self.box_extents = extents;
        self
    }
}

impl Paragraph {
    /// Вырезы обтекания (`shape-outside`).
    pub fn flow_shapes(
        mut self,
        flow: std::sync::Arc<(
            Vec<crate::layout::float::shapes::FloatShape>,
            Vec<crate::layout::float::shapes::FloatShape>,
        )>,
    ) -> Self {
        self.flow = flow;
        self
    }
}
