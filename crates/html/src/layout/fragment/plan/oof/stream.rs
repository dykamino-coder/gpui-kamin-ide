//! Исполнитель очереди OOF: normal flow задаёт только пространства и исходные якоря.

use super::Flow;
use crate::layout::fragment::flow::inline::Geometry;
use crate::layout::fragment::token::resume::OofResume;
use crate::layout::fragment::token::resume::oof::batch::{Batch, resume_all};
use crate::layout::fragment::token::{Error, Space, TokenStore};
use crate::layout::fragment::types::{Frag, Kid};

pub(crate) struct Input {
    pub(crate) kid: Kid,
    pub(crate) inline: Geometry,
    /// Fragmentainer исходного CB/static anchor; не меняется при продолжении.
    pub(crate) first: usize,
    pub(crate) block_start: f32,
    pub(crate) inline_start: f32,
    pub(crate) repeat_fixed: bool,
    pub(crate) fixed_content: bool,
}

/// Все тяжёлые значения находятся в векторах, а не в рекурсивных кадрах сборки.
pub(crate) struct Streams {
    inputs: Vec<Input>,
    flows: Vec<Flow>,
    pending: Vec<OofResume>,
    waiting: Vec<(usize, OofResume)>,
    store: TokenStore,
    next: usize,
}

impl Streams {
    pub(crate) fn new(inputs: Vec<Input>) -> Result<Self, Error> {
        let waiting = inputs
            .iter()
            .enumerate()
            .map(|(idx, input)| {
                Ok((
                    input.first,
                    OofResume {
                        idx: u32::try_from(idx).map_err(|_| Error::ResourceExhausted)?,
                        block_start: input.block_start,
                        inline_start: input.inline_start,
                        repeat_fixed: input.repeat_fixed,
                        token: None,
                    },
                ))
            })
            .collect::<Result<Vec<_>, Error>>()?;
        Ok(Self {
            flows: (0..inputs.len()).map(|_| Flow::default()).collect(),
            inputs,
            pending: Vec::new(),
            waiting,
            store: TokenStore::default(),
            next: 0,
        })
    }

    pub(crate) fn needs_space(&self) -> bool {
        self.pending.iter().any(|p| !p.repeat_fixed)
            || self.waiting.iter().any(|(_, p)| !p.repeat_fixed)
    }

    /// Повтор fixed заново раскладывается в CB своей страницы; abspos не меняется.
    pub(crate) fn refresh_fixed(&mut self, index: usize, input: Input) -> Result<(), Error> {
        let old = self.inputs.get_mut(index).ok_or(Error::InvalidToken)?;
        if !old.repeat_fixed || !input.repeat_fixed {
            return Err(Error::InvalidToken);
        }
        for pending in self
            .pending
            .iter_mut()
            .chain(self.waiting.iter_mut().map(|(_, p)| p))
        {
            if pending.idx as usize == index {
                pending.block_start = input.block_start;
                pending.inline_start = input.inline_start;
            }
        }
        *old = input;
        Ok(())
    }

    /// Один реальный fragmentainer. Ни размер, ни повторный CB здесь не вычисляются.
    pub(crate) fn resume(
        &mut self,
        index: usize,
        space: Space,
        normal_complete: bool,
    ) -> Result<Batch<Frag>, Error> {
        if index != self.next {
            return Err(Error::InvalidToken);
        }
        let mut waiting = Vec::new();
        for (first, input) in self.waiting.drain(..) {
            if first <= index {
                self.pending.push(input);
            } else {
                waiting.push((first, input));
            }
        }
        self.waiting = waiting;
        self.pending.sort_by_key(|p| p.idx);
        let normal_complete = normal_complete && !self.waiting.iter().any(|(_, p)| !p.repeat_fixed);
        let inputs = &self.inputs;
        let flows = &mut self.flows;
        let mut batch = resume_all(
            &self.pending,
            space,
            normal_complete,
            &mut self.store,
            |idx, space| {
                if inputs[idx as usize].fixed_content
                    && space.is_paginated()
                    && space.may_break_inside()
                {
                    Space {
                        repeat: crate::layout::fragment::token::space::RepeatState::Inside,
                        ..space
                    }
                } else {
                    space
                }
            },
            |idx, start, token, space, store| {
                let input = inputs.get(idx as usize).ok_or(Error::InvalidToken)?;
                let mut child = flows[idx as usize].resume(
                    &input.kid,
                    &input.inline,
                    space,
                    start,
                    token,
                    store,
                )?;
                if let Some(fragment) = &mut child.fragment {
                    fragment.col = index;
                }
                Ok(child)
            },
        )?;
        self.pending.clone_from(&batch.next);
        batch.needs_space = self.needs_space();
        self.next = self.next.checked_add(1).ok_or(Error::ResourceExhausted)?;
        Ok(batch)
    }
}

#[cfg(test)]
mod tests;
