//! Битовые множества элементов: пересечение словами и обход только установленных битов.

use crate::budget::admission::{Admission, Buffer, Error};

pub(super) struct Bits(Buffer<u64>);

impl Bits {
    pub fn new(count: usize, admission: &Admission) -> Result<Self, Error> {
        let words = count.div_ceil(64);
        let mut out = Buffer::default();
        out.reserve(words, admission)?;
        for _ in 0..words {
            out.push(0, admission)?;
        }
        admission.work(words)?;
        Ok(Self(out))
    }

    pub fn insert(&mut self, i: usize) {
        self.0[i / 64] |= 1 << (i % 64);
    }

    pub fn contains(&self, i: usize) -> bool {
        self.0[i / 64] & (1 << (i % 64)) != 0
    }

    pub fn copy(&self, admission: &Admission) -> Result<Self, Error> {
        let mut out = Self::new(self.0.len() * 64, admission)?;
        out.0.copy_from_slice(&self.0);
        Ok(out)
    }

    pub fn is_empty(&self) -> bool {
        self.0.iter().all(|&word| word == 0)
    }

    /// Равные непрерывные срезы позволяют LLVM векторизовать независимые AND.
    pub fn intersect(&mut self, other: &Self, admission: &Admission) -> Result<(), Error> {
        admission.work(self.0.len())?;
        for (left, right) in self.0.iter_mut().zip(other.0.iter()) {
            *left &= *right;
        }
        Ok(())
    }

    pub fn ones(&self) -> impl Iterator<Item = usize> + '_ {
        self.0.iter().enumerate().flat_map(|(at, &word)| {
            let mut remaining = word;
            std::iter::from_fn(move || {
                if remaining == 0 {
                    return None;
                }
                let bit = remaining.trailing_zeros() as usize;
                remaining &= remaining - 1;
                Some(at * 64 + bit)
            })
        })
    }
}
