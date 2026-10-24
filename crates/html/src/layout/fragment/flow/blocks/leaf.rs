//! Ограниченная раскладка пустого блока: заданный content block-size продолжается токеном.
//! Декорации и независимые потоки сюда не входят; процентную базу решает исходная раскладка.

use crate::layout::fragment::token::cursor::ChildCursor;
use crate::layout::fragment::token::fc::{BlockState, FcState};
use crate::layout::fragment::token::{Error, Space, Token, TokenId, TokenStore};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Leaf {
    pub(crate) content_block: f32,
}

#[derive(Debug, PartialEq)]
pub(crate) struct Fragment {
    pub(crate) from: f32,
    pub(crate) block_size: f32,
}

#[derive(Debug, PartialEq)]
pub(crate) struct Output {
    pub(crate) fragment: Option<Fragment>,
    pub(crate) next: Option<TokenId>,
}

impl Leaf {
    /// `offset` относится к текущему фрагментаинеру, не к процентному CB.
    pub(crate) fn resume(
        self,
        space: Space,
        offset: f32,
        input: Option<TokenId>,
        store: &mut TokenStore,
    ) -> Result<Output, Error> {
        let available = space.block_size.ok_or(Error::InvalidGeometry)?;
        if ![self.content_block, available]
            .iter()
            .all(|n| n.is_finite() && *n >= 0.0)
            || !offset.is_finite()
        {
            return Err(Error::InvalidGeometry);
        }
        let previous = input
            .map(|id| store.get(id).ok_or(Error::InvalidToken))
            .transpose()?;
        let from = previous.map_or(0.0, |t| t.consumed);
        if let Some(token) = previous {
            let FcState::Block(state) = &token.fc else {
                return Err(Error::InvalidToken);
            };
            if from < 0.0
                || from > self.content_block
                || !from.is_finite()
                || (token.flags.break_before && (from != 0.0 || token.seq != 0))
                || ChildCursor::new(0, state, token.flags.seen_all_children)?
                    .next()
                    .is_some()
                || !token.oof.is_empty()
                || !state.float_exclusions.is_empty()
                || token.clamp.is_some()
                || token.monolithic_overflow != 0.0
                || token.flags.repeated
                || token.flags.by_spanner
            {
                return Err(Error::InvalidToken);
            }
        }
        let rest = self.content_block - from;
        let room = (available - offset).max(0.0);
        if rest > 0.0 && room == 0.0 {
            if offset == 0.0 {
                return Err(Error::NoProgress);
            }
            let mut token = previous.cloned().unwrap_or_else(|| {
                let mut t = Token::new(FcState::Block(BlockState::default()));
                t.seq = 0;
                t
            });
            // Отложено начало; у продолжения сохраняются consumed и sequence.
            token.flags.break_before = from == 0.0;
            return Ok(Output {
                fragment: None,
                next: Some(store.push(token)?),
            });
        }
        let block_size = rest.min(room);
        let consumed = from + block_size;
        if rest > 0.0 && consumed <= from {
            return Err(Error::NoProgress);
        }
        let next = if consumed < self.content_block {
            let fc = FcState::Block(BlockState::default());
            let mut token = match previous {
                Some(t) => t.advance(block_size, fc)?,
                None => {
                    let mut t = Token::new(fc);
                    t.consumed = block_size;
                    t
                }
            };
            token.flags.seen_all_children = true;
            token.at_block_end = true;
            Some(store.push(token)?)
        } else {
            None
        };
        Ok(Output {
            fragment: Some(Fragment { from, block_size }),
            next,
        })
    }
}

#[cfg(test)]
mod tests;
