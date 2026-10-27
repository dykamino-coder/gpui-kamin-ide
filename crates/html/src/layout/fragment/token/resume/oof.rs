//! Исполнение независимого OOF: отсрочка расходует фактические Space, повтор не рождает листы.

use super::OofResume;
use crate::layout::fragment::token::space::RepeatState;
use crate::layout::fragment::token::{Error, Space, TokenId, TokenStore};
pub(crate) mod batch;
mod progress;

/// Политику определяет ближайший CB, а не наличие transform выше него.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FixedPolicy {
    Fragment,
    RepeatPages,
}

impl FixedPolicy {
    pub(crate) fn for_box(fixed: bool, fixed_cb: bool) -> Self {
        if fixed && !fixed_cb {
            Self::RepeatPages
        } else {
            Self::Fragment
        }
    }
}

/// Результат ребёнка: None означает завершение именно его потока.
pub(crate) struct Child<T> {
    pub(crate) fragment: Option<T>,
    pub(crate) next: Option<TokenId>,
}

pub(crate) struct Output<T> {
    pub(crate) fragment: Option<T>,
    pub(crate) next: Option<OofResume>,
}

impl<T> Output<T> {
    /// Повтор fixed обслуживает уже существующие страницы.
    pub(crate) fn needs_space(&self) -> bool {
        self.next.is_some_and(|next| !next.repeat_fixed)
    }
}

/// CB/static anchor остаются у владельца потока; здесь меняются только pending start и child token.
pub(crate) fn resume<T>(
    mut input: OofResume,
    space: Space,
    store: &mut TokenStore,
    child: impl FnOnce(f32, Option<TokenId>, &mut TokenStore) -> Result<Child<T>, Error>,
) -> Result<Output<T>, Error> {
    let size = space.block_size.ok_or(Error::InvalidGeometry)?;
    if !size.is_finite()
        || size <= 0.
        || !input.block_start.is_finite()
        || !input.inline_start.is_finite()
    {
        return Err(Error::InvalidGeometry);
    }
    let previous = input
        .token
        .map(|id| store.get(id).cloned().ok_or(Error::InvalidToken))
        .transpose()?;
    if previous
        .as_ref()
        .is_some_and(|token| !token.consumed.is_finite() || token.consumed < 0.)
    {
        return Err(Error::InvalidGeometry);
    }
    let repeated = input.repeat_fixed;
    if repeated && (!space.is_paginated() || !matches!(space.repeat, RepeatState::Repeat { .. })) {
        return Err(Error::InvalidGeometry);
    }
    if !repeated && space.may_break_inside() && input.token.is_none() && input.block_start >= size {
        let remaining = input.block_start - size;
        if remaining >= input.block_start {
            return Err(Error::NoProgress);
        }
        input.block_start = remaining;
        return Ok(Output {
            fragment: None,
            next: Some(input),
        });
    }
    let start = if input.token.is_some() && !repeated {
        0.
    } else {
        input.block_start
    };
    let result = child(start, if repeated { None } else { input.token }, store)?;
    if let Some(next) = result.next {
        let token = store.get(next).ok_or(Error::InvalidToken)?;
        if !token.consumed.is_finite() || token.consumed < 0. {
            return Err(Error::InvalidGeometry);
        }
        if repeated || !space.may_break_inside() {
            return Err(Error::NoProgress);
        }
        if let Some(previous) = &previous {
            if !progress::advances(previous, token, store)
                && !(token.flags.break_before && (start > 0. || !space.at_start(0)))
            {
                return Err(Error::NoProgress);
            }
        } else if result.fragment.is_none()
            && token.consumed == 0.
            && !(token.flags.break_before && (start > 0. || !space.at_start(0)))
        {
            return Err(Error::NoProgress);
        }
    }
    let next = if repeated {
        (!matches!(space.repeat, RepeatState::Repeat { last: true })).then_some(input)
    } else {
        result.next.map(|token| {
            input.token = Some(token);
            input.block_start = 0.;
            input
        })
    };
    Ok(Output {
        fragment: result.fragment,
        next,
    })
}

#[cfg(test)]
mod tests;
