//! Битовые множества элементов: пересечение словами и обход только установленных битов.

use crate::budget::admission::{Admission, Buffer, Error};

pub(super) struct Bits(Buffer<u64>);

impl Bits {
    pub fn empty() -> Self {
        Self(Buffer::default())
    }

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
        self.0
            .get(i / 64)
            .is_some_and(|word| word & (1 << (i % 64)) != 0)
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
        let shared = self.0.len().min(other.0.len());
        for (left, right) in self.0[..shared].iter_mut().zip(other.0.iter()) {
            *left &= *right;
        }
        self.0[shared..].fill(0);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intersection_with_empty_set_clears_every_word_in_both_orders() {
        let admission = Admission::default();
        let mut populated = Bits::new(130, &admission).unwrap();
        for i in [0, 63, 64, 129] {
            populated.insert(i);
        }
        let mut empty = Bits::empty();
        empty.intersect(&populated, &admission).unwrap();
        assert!(empty.is_empty());
        populated.intersect(&empty, &admission).unwrap();
        assert!(populated.is_empty());
        assert!(populated.ones().next().is_none());
        assert!(!empty.contains(129));
    }
}
