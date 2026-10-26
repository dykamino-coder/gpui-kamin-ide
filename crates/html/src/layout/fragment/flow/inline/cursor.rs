//! Токен абзаца принадлежит его IFC и сохраняется при перестроении Element.
use crate::layout::fragment::token::Token;
use crate::layout::fragment::token::fc::{FcState, InlineState};
use std::cell::RefCell;

#[derive(Debug, Default)]
pub(crate) struct Cursor(RefCell<Option<Token>>);

impl Cursor {
    pub(crate) fn state(&self) -> Option<InlineState> {
        self.0.borrow().as_ref().and_then(|token| match &token.fc {
            FcState::Inline(state) => Some(state.clone()),
            _ => None,
        })
    }
    pub(crate) fn get(&self) -> usize {
        self.state().map_or(0, |state| state.text_pos)
    }
    pub(crate) fn reset(&self) {
        *self.0.borrow_mut() = None;
    }
    pub(crate) fn consumed(&self) -> f32 {
        self.0.borrow().as_ref().map_or(0.0, |t| t.consumed)
    }
    #[cfg(test)]
    pub(crate) fn resume(&self, state: InlineState) {
        self.resume_at(state, 0.0);
    }
    pub(crate) fn resume_at(&self, state: InlineState, consumed: f32) {
        let mut token = Token::new(FcState::Inline(state));
        token.consumed = consumed;
        *self.0.borrow_mut() = Some(token);
    }
}
