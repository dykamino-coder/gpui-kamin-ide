//! Физическая раскладка текста внутри одного строчного контекста. Проекция меняет
//! только used-стиль коробки: computed оси и шрифтовые единицы элемента сохраняются.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InlineFormat {
    Turned,
    Upright,
    Combined,
}

impl BoxStyle {
    /// Содержимое той же строки использует её физические оси; атомарная коробка
    /// и внепоточный элемент начинают собственную раскладку (css-display-3 §2).
    pub fn inline_contents(&mut self, parent: &BoxStyle, unit: bool) {
        let inline = matches!(
            self.reset_box_layout.display,
            None | Some(Display::Contents)
        ) && !matches!(
            self.reset_positioned.position,
            Some(Position::Absolute | Position::Fixed)
        );
        if !(inline || unit) {
            return;
        }
        if parent.reset_text_box.ruby_unit {
            self.inh.text_flow.nowrap = Some(true);
        }
        let Some(format) = parent.inline_format else {
            return;
        };
        self.inline_format = Some(format);
        self.inh.writing.flat_mode = parent.inh.writing.flat_mode;
        self.inh.writing.writing_mode = None;
        self.reset_text_box.turned_line = parent.reset_text_box.turned_line;
        self.reset_context.lines_reversed = parent.reset_context.lines_reversed;
        let em = match self.inh.font.font_size {
            Some(Len::Px(px)) => px,
            _ => 16.0,
        };
        match format {
            InlineFormat::Turned => {}
            InlineFormat::Upright => {
                self.reset_context.upright_stack = true;
                self.inh.text_flow.break_word = Some(true);
                self.inh.font.line_height = Some(Len::Px(em));
            }
            InlineFormat::Combined => {
                self.inh.writing.combine_upright = Some(None);
                self.inh.text_flow.letter_spacing = Some(Len::Px(0.0));
                self.inh.font.line_height = Some(Len::Px(em));
            }
        }
    }
}
