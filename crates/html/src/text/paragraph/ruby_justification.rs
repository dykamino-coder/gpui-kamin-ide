//! Выключка внутри руби: `text-justify: ruby` отключает раздачу остатка по
//! пробелам в единицах руби (css-text-4 §7.3), и без точек раздачи
//! `space-between` центрирует содержимое (css-ruby-1 §4.3); здесь же
//! признак общей повёрнутой растровой рамки руби и соседнего текста.

use super::{Align, Paragraph};

impl Paragraph {
    pub fn ruby_justify(
        mut self,
        enabled: bool,
        ruby_unit: bool,
        rotated: bool,
        has_ruby: bool,
    ) -> Self {
        self.ruby_justify = enabled;
        self.ruby_unit = ruby_unit;
        // Ruby and its ordinary text siblings share one transformed raster
        // frame; snapping only the ordinary words before rotation splits it.
        self.ruby_rotated = (ruby_unit || has_ruby) && rotated;
        self
    }

    pub(super) fn ruby_line_align(&self, align: Align, range: &std::ops::Range<usize>) -> Align {
        // CSS Ruby 1 §4.3: absent justification opportunities, space-between
        // centers the content. Word separators supply none under text-justify:
        // ruby; their natural advances are still retained by the shaped line.
        if self.ruby_justify
            && self.ruby_unit
            && align == Align::Justify
            && self
                .words(range)
                .last()
                .is_none_or(|word| word.spaces_before == 0)
        {
            Align::Center
        } else {
            align
        }
    }
}
