//! Множество длинных свойств реестра — битовая карта по `LonghandId` (Blink `CSSBitset`).

use super::LonghandId;

const WORDS: usize = 4;
const _: () = assert!(LonghandId::ALL.len() <= WORDS * 64);

/// Множество длинных свойств реестра.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LonghandSet([u64; WORDS]);

impl LonghandSet {
    fn bit(id: LonghandId) -> (usize, u64) {
        let i = id as usize;
        (i / 64, 1 << (i % 64))
    }

    pub fn contains(&self, id: LonghandId) -> bool {
        let (word, bit) = Self::bit(id);
        self.0[word] & bit != 0
    }

    pub fn set(&mut self, id: LonghandId, on: bool) {
        let (word, bit) = Self::bit(id);
        if on {
            self.0[word] |= bit;
        } else {
            self.0[word] &= !bit;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.0.iter().all(|w| *w == 0)
    }

    /// Свойства множества в порядке таблицы реестра.
    pub fn iter(self) -> impl Iterator<Item = LonghandId> {
        LonghandId::ALL
            .iter()
            .copied()
            .filter(move |id| self.contains(*id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn membership_follows_set_and_clear() {
        let mut s = LonghandSet::default();
        assert!(s.is_empty());
        let last = *LonghandId::ALL.last().unwrap();
        s.set(LonghandId::MarginTop, true);
        s.set(last, true);
        assert!(s.contains(LonghandId::MarginTop) && s.contains(last));
        assert_eq!(s.iter().collect::<Vec<_>>(), [LonghandId::MarginTop, last]);
        s.set(LonghandId::MarginTop, false);
        assert!(!s.contains(LonghandId::MarginTop));
        s.set(last, false);
        assert!(s.is_empty());
    }
}
