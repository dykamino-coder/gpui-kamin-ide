//! Разреженный результат аргумента и отдельная мемоизация каждой альтернативы.

use super::*;
use crate::budget::admission::{Admission, Buffer};

pub(super) struct Marks {
    seen: Buffer<usize>,
    union: Buffer<bool>,
    pub touched: Buffer<usize>,
    generation: usize,
}

impl Marks {
    pub fn new(count: usize, admission: &Admission) -> Result<Self, Error> {
        let mut seen = Buffer::default();
        let mut union = Buffer::default();
        for _ in 0..count {
            seen.push(0, admission)?;
            union.push(false, admission)?;
        }
        Ok(Self {
            seen,
            union,
            touched: Buffer::default(),
            generation: 0,
        })
    }

    pub fn argument(&mut self) {
        while let Some(i) = self.touched.pop() {
            self.union[i] = false;
        }
    }

    pub fn alternative(&mut self) -> Result<(), Error> {
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or(Error::CapacityOverflow)?;
        Ok(())
    }

    pub fn contains(&self, i: usize) -> bool {
        self.union[i]
    }

    pub fn propagate(&mut self, i: usize, admission: &Admission) -> Result<bool, Error> {
        if self.seen[i] == self.generation {
            return Ok(false);
        }
        self.seen[i] = self.generation;
        if !self.union[i] {
            self.touched.push(i, admission)?;
            self.union[i] = true;
        }
        Ok(true)
    }
}
