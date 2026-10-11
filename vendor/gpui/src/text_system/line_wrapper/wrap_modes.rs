//! Editor text keeps its wrapping contract; HTML explicitly opts into CSS rules.
use super::{Boundary, IndentAdjustment, LineFragment, LineWrapper};
use crate::Pixels;

impl LineWrapper {
    /// Wrap editor text, including leading whitespace and whitespace-only lines.
    pub fn wrap_line<'a>(
        &'a mut self,
        fragments: &'a [LineFragment],
        wrap_width: Pixels,
        indent_adjustment: IndentAdjustment,
    ) -> impl Iterator<Item = Boundary> + 'a {
        self.wrap_line_policy(fragments, wrap_width, indent_adjustment, false)
    }

    /// Wrap HTML text with hanging spaces and CSS no-break punctuation/glue.
    pub fn wrap_line_css<'a>(
        &'a mut self,
        fragments: &'a [LineFragment],
        wrap_width: Pixels,
    ) -> impl Iterator<Item = Boundary> + 'a {
        // Отступ продолжения — как прежде (до upstream `IndentAdjustment`):
        // ведущие пробелы строки.
        self.wrap_line_policy(fragments, wrap_width, IndentAdjustment::SameIndent, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TestAppContext, TestDispatcher, font, px};

    #[test]
    fn editor_spaces_wrap_but_css_spaces_hang() {
        let cx = TestAppContext::build(TestDispatcher::new(0), None);
        let id = cx.text_system().resolve_font(&font(".ZedMono"));
        let mut wrapper = LineWrapper::new(id, px(16.), cx.text_system().clone());
        let fragments = [LineFragment::text("                            ")];
        assert_eq!(
            wrapper
                .wrap_line(&fragments, px(72.), IndentAdjustment::default())
                .collect::<Vec<_>>(),
            vec![
                Boundary::new(7, 0),
                Boundary::new(14, 0),
                Boundary::new(21, 0)
            ]
        );
        assert!(wrapper.wrap_line_css(&fragments, px(72.)).next().is_none());
    }

    #[test]
    fn css_punctuation_does_not_change_editor_word_classification() {
        // upstream отнёс `)` и `!` к словесным и для редактора (UAX #14 LB13),
        // поэтому проверяются знаки, которые у редактора остались разрывом.
        for c in ['/', '?', '\u{2060}'] {
            assert!(!LineWrapper::is_word_char(c));
            assert!(LineWrapper::is_css_word_char(c));
        }
    }
}
