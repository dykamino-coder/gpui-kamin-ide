//! Обход продолжений детей перед ещё не начатыми детьми; один ребёнок выдаётся один раз.

use super::fc::{BlockState, ChildToken};
use super::{Error, TokenId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Child {
    pub(crate) index: u32,
    pub(crate) token: Option<TokenId>,
}

pub(crate) struct ChildCursor<'a> {
    main: Option<ChildToken>,
    parallel: std::slice::Iter<'a, ChildToken>,
    next: u32,
    count: u32,
}

impl<'a> ChildCursor<'a> {
    pub(crate) fn new(count: u32, state: &'a BlockState, seen_all: bool) -> Result<Self, Error> {
        let next = state
            .child_idx
            .checked_add(u32::from(state.child.is_some()))
            .ok_or(Error::InvalidToken)?;
        if next > count || (seen_all && next != count) {
            return Err(Error::InvalidToken);
        }
        let mut previous = None;
        for child in &state.parallel {
            if child.idx >= next
                || (state.child.is_some() && child.idx == state.child_idx)
                || previous.is_some_and(|p| child.idx <= p)
            {
                return Err(Error::InvalidToken);
            }
            previous = Some(child.idx);
        }
        Ok(Self {
            main: state.child.map(|token| ChildToken {
                idx: state.child_idx,
                token,
            }),
            parallel: state.parallel.iter(),
            next,
            count,
        })
    }
}

impl Iterator for ChildCursor<'_> {
    type Item = Child;
    fn next(&mut self) -> Option<Self::Item> {
        let pending = match (self.main, self.parallel.as_slice().first()) {
            (Some(main), other) if other.is_none_or(|p| main.idx < p.idx) => self.main.take(),
            (_, Some(_)) => self.parallel.next().copied(),
            _ => None,
        };
        if let Some(child) = pending {
            return Some(Child {
                index: child.idx,
                token: Some(child.token),
            });
        }
        if self.next == self.count {
            return None;
        }
        let index = self.next;
        self.next += 1;
        Some(Child { index, token: None })
    }
}
