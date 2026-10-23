//! Физическая раскладка текста внутри одного строчного контекста. Проекция меняет
//! только used-стиль коробки: computed оси и шрифтовые единицы элемента сохраняются.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InlineFormat {
    Turned,
    Upright,
    Combined,
}

impl BoxStyle {
    /// Содержимое той же строки использует её физические оси; атомарная коробка
    /// и внепоточный элемент начинают собственную раскладку (css-display-3 §2).
    pub(crate) fn inline_contents(&mut self, parent: &BoxStyle, unit: bool) {
        let inline = matches!(self.display, None | Some(Display::Contents))
            && !matches!(self.position, Some(Position::Absolute | Position::Fixed));
        if !(inline || unit) {
            return;
        }
        if parent.ruby_unit {
            self.nowrap = Some(true);
        }
        let Some(format) = parent.inline_format else {
            return;
        };
        self.inline_format = Some(format);
        self.flat_mode = parent.flat_mode;
        self.writing_mode = None;
        self.turned_line = parent.turned_line;
        self.lines_reversed = parent.lines_reversed;
        let em = match self.font_size {
            Some(Len::Px(px)) => px,
            _ => 16.0,
        };
        match format {
            InlineFormat::Turned => {}
            InlineFormat::Upright => {
                self.upright_stack = true;
                self.break_word = Some(true);
                self.line_height = Some(Len::Px(em));
            }
            InlineFormat::Combined => {
                self.combine_upright = Some(None);
                self.letter_spacing = Some(Len::Px(0.0));
                self.line_height = Some(Len::Px(em));
            }
        }
    }
}
