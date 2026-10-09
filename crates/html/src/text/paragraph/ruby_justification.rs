//! Word-space justification is disabled for ruby annotations (CSS Text 4 §7.3).

use super::{Align, Paragraph};

impl Paragraph {
    pub fn ruby_justify(mut self, enabled: bool, ruby_unit: bool) -> Self {
        self.ruby_justify = enabled;
        self.ruby_unit = ruby_unit;
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
