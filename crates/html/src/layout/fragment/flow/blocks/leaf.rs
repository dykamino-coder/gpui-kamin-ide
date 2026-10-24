//! Ограниченная раскладка пустого блока: заданный content block-size продолжается токеном.
//! Края расходуют место отдельно; процентную базу решает исходная раскладка.

use crate::layout::fragment::token::cursor::ChildCursor;
use crate::layout::fragment::token::fc::{BlockState, FcState};
use crate::layout::fragment::token::{Error, Space, Token, TokenId, TokenStore};

mod edges;
pub(crate) use edges::Edges;
mod piece;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Leaf {
    pub(crate) content_block: f32,
    pub(crate) edges: Edges,
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
        if !self.edges.valid()
            || ![self.content_block, available]
                .iter()
                .all(|n| n.is_finite() && *n >= 0.0)
            || !offset.is_finite()
        {
            return Err(Error::InvalidGeometry);
        }
        let previous = input
            .map(|id| store.get(id).ok_or(Error::InvalidToken))
            .transpose()?;
        let mut from = previous.map_or(0.0, |t| t.consumed);
        let mut sliced = (0.0, 0.0);
        if let Some(token) = previous {
            let FcState::Block(state) = &token.fc else {
                return Err(Error::InvalidToken);
            };
            sliced = state
                .sliced_edges
                .unwrap_or((if token.flags.break_before { 0.0 } else { 1.0 }, 0.0));
            if ![sliced.0, sliced.1].iter().all(|v| (0.0..=1.0).contains(v)) {
                return Err(Error::InvalidToken);
            }
            if let Some(size) = state.content_block_size {
                if !size.is_finite() || size < 0.0 || from < 0.0 || from > size {
                    return Err(Error::InvalidToken);
                }
                from = if size == self.content_block {
                    from
                } else if size == 0.0 {
                    0.0
                } else {
                    from / size * self.content_block
                };
            }
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
        let first = previous.is_none_or(|t| t.flags.break_before);
        let start = self.edges.start_at(first);
        let rest = self.content_block - from;
        let room = (available - offset).max(0.0);
        let defer_edges =
            offset > 0.0 && self.edges.clone && start + self.edges.end >= room && rest > 0.0;
        if (rest + start + self.edges.end > 0.0 && room == 0.0) || defer_edges {
            if offset == 0.0 {
                return Err(Error::NoProgress);
            }
            let mut token = previous.cloned().unwrap_or_else(|| {
                let mut t = Token::new(FcState::Block(BlockState::default()));
                t.seq = 0;
                t
            });
            token.consumed = from;
            token.fc = FcState::Block(BlockState {
                content_block_size: Some(self.content_block),
                sliced_edges: (!self.edges.clone).then_some(sliced),
                ..BlockState::default()
            });
            // Отложено начало; у продолжения сохраняются consumed и sequence.
            token.flags.break_before = first;
            return Ok(Output {
                fragment: None,
                next: Some(store.push(token)?),
            });
        }
        let piece = piece::Piece::new(self, from, first, sliced, room);
        let take = piece.take;
        let consumed = from + take;
        if !piece.complete && (piece.fragment.block_size == 0.0 || (take > 0.0 && consumed <= from))
        {
            return Err(Error::NoProgress);
        }
        let next = if !piece.complete {
            let fc = FcState::Block(BlockState {
                content_block_size: Some(self.content_block),
                sliced_edges: piece.sliced,
                ..BlockState::default()
            });
            let mut token = match previous {
                Some(t) => {
                    let mut t = t.clone();
                    t.consumed = from;
                    t.advance(take, fc)?
                }
                None => {
                    let mut t = Token::new(fc);
                    t.consumed = take;
                    t
                }
            };
            token.flags.seen_all_children = true;
            token.flags.break_before = false;
            token.at_block_end = consumed == self.content_block;
            Some(store.push(token)?)
        } else {
            None
        };
        Ok(Output {
            fragment: Some(piece.fragment),
            next,
        })
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod edge_tests;
