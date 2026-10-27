//! Независимый шаг OOF использует общий выбор разрыва normal flow.

use super::cuts::InlineTokens;
use super::{Plan, pieces::fill_pieces_inner};
use crate::layout::fragment::flow::inline::Geometry;
use crate::layout::fragment::token::fc::{BlockState, FcState};
use crate::layout::fragment::token::resume::oof::Child;
use crate::layout::fragment::token::{Error, Space, Token, TokenId, TokenStore};
use crate::layout::fragment::types::{Frag, Kid};

pub(crate) mod stream;

#[derive(Default)]
pub(crate) struct Flow {
    inline: InlineTokens,
    pub(crate) shortage: Option<f32>,
}

impl Flow {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn resume(
        &mut self,
        kid: &Kid,
        inline: &Geometry,
        space: Space,
        start: f32,
        input: Option<TokenId>,
        store: &mut TokenStore,
    ) -> Result<Child<Frag>, Error> {
        self.shortage = None;
        let size = space.block_size.ok_or(Error::InvalidGeometry)?;
        let previous = input
            .map(|id| store.get(id).cloned().ok_or(Error::InvalidToken))
            .transpose()?;
        let from = previous.as_ref().map_or(0., |t| t.consumed);
        let copy = match previous.as_ref().map(|t| &t.fc) {
            None => 0,
            Some(FcState::Block(_)) => previous.as_ref().map_or(0, |token| token.seq as usize),
            _ => return Err(Error::InvalidToken),
        };
        let end = kid.h.max(kid.over);
        if !size.is_finite()
            || size <= 0.
            || !start.is_finite()
            || !end.is_finite()
            || !from.is_finite()
            || from < 0.
            || end < from
        {
            return Err(Error::InvalidGeometry);
        }
        if !space.may_break_inside() {
            if input.is_some() {
                return Err(Error::InvalidToken);
            }
            return Ok(Child {
                fragment: Some(Frag {
                    kid: 0,
                    copy: 0,
                    col: 0,
                    y: start,
                    from: 0.,
                    h: end,
                    head: 0.,
                    foot: 0.,
                }),
                next: None,
            });
        }
        let mut plan = Plan::default();
        let mut col = 0;
        let mut y = 0.;
        let mut placed = start > 0. || !space.at_start(0);
        let mut shortage = f32::MAX;
        let cursor = fill_pieces_inner(
            &|_| size,
            usize::MAX,
            space.is_paginated(),
            &mut col,
            &mut y,
            space.not_at_start,
            &mut placed,
            &mut shortage,
            &mut plan,
            &mut None,
            0,
            kid,
            start,
            from,
            copy,
            end,
            kid.repeat,
            &mut |_| super::Measurement {
                copy: None,
                inline: inline.clone(),
            },
            &mut self.inline,
            true,
        )?;
        self.shortage = (shortage < f32::MAX && shortage > 0.).then_some(shortage);
        let fragment = plan.fragments.into_iter().next();
        let next = if cursor.complete {
            None
        } else {
            let fc = FcState::Block(BlockState::default());
            if cursor.from < from || (fragment.is_some() && cursor.from == from) {
                return Err(Error::NoProgress);
            }
            let seq = u32::try_from(cursor.copy).map_err(|_| Error::ResourceExhausted)?;
            let mut token = previous.unwrap_or_else(|| Token::new(fc.clone()));
            token.consumed = cursor.from;
            token.fc = fc;
            token.seq = seq;
            token.at_block_end = cursor.from >= kid.h;
            token.flags.break_before = fragment.is_none();
            Some(store.push(token)?)
        };
        Ok(Child { fragment, next })
    }
}

#[cfg(test)]
mod tests;
