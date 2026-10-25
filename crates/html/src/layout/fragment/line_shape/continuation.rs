//! Адрес следующей строки относится к полному тексту IFC, а не к визуальному bidi-порядку.
use crate::layout::fragment::token::{Error, fc::InlineState};

pub(crate) fn line_state(
    text: &str,
    index: usize,
    byte: usize,
    after_hard_break: bool,
) -> Result<InlineState, Error> {
    text.get(..byte).ok_or(Error::InvalidToken)?;
    Ok(InlineState {
        line_idx: index,
        text_pos: byte,
        past_first_line: index > 0,
        after_hard_break,
        child: None,
    })
}
