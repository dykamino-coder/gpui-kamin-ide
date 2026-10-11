//! CSS no-break classes are distinct from the editor's word classification.
use super::LineWrapper;

impl LineWrapper {
    pub(crate) fn is_css_word_char(c: char) -> bool {
        Self::is_word_char(c) || Self::is_no_break_before(c)
    }
    /// KaminIDE patch: перед этими знаками нельзя переносить НИКОГДА —
    /// UAX #14 LB11 (`× WJ`) и LB12a (`[^SP BA HY] × GL`).
    pub(super) fn is_glue_before(c: char) -> bool {
        use unicode_linebreak::BreakClass::*;
        matches!(
            unicode_linebreak::break_property(c as u32),
            NonBreakingGlue | WordJoiner
        )
    }

    /// KaminIDE patch: a line may not END with these (UAX #14 class OP/GL).
    ///
    /// Every non-word character is a wrap candidate above, so without this an
    /// opening bracket would be left dangling at the end of a line.
    pub(crate) fn is_no_break_after(c: char) -> bool {
        use unicode_linebreak::BreakClass::*;
        matches!(
            unicode_linebreak::break_property(c as u32),
            OpenPunctuation | NonBreakingGlue | WordJoiner
        )
    }

    /// KaminIDE patch: a line may not START with these (UAX #14 CL/CP/EX/IS/NS/SY).
    fn is_no_break_before(c: char) -> bool {
        use unicode_linebreak::BreakClass::*;
        matches!(
            unicode_linebreak::break_property(c as u32),
            ClosePunctuation
                | CloseParenthesis
                | Exclamation
                | InfixSeparator
                | NonStarter
                | Symbol
                | NonBreakingGlue
                | WordJoiner
        )
    }
}
