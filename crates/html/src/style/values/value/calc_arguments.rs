//! Потоковые однородные аргументы функций сравнения и вариадических математических функций.

use super::calc::Val;
use super::calc_math::{key, rebuild};

#[derive(Default)]
pub(super) struct Homogeneous {
    nature: Option<u8>,
    zero_length: bool,
}

impl Homogeneous {
    pub(super) fn push(&mut self, value: Val) -> Option<f32> {
        let (nature, coefficient) = key(&value)?;
        let zero = matches!(value, Val::Len(_)) && nature == 0 && coefficient == 0.0;
        if let Some(previous) = self.nature {
            if previous != nature {
                if self.zero_length && nature < u8::MAX - 1 {
                    self.nature = Some(nature);
                } else if !(zero && previous < u8::MAX - 1) {
                    return None;
                }
            }
            self.zero_length &= zero;
        } else {
            self.nature = Some(nature);
            self.zero_length = zero;
        }
        Some(coefficient)
    }

    pub(super) fn nature(&self) -> Option<u8> {
        self.nature
    }

    pub(super) fn rebuild(&self, coefficient: f32) -> Option<Val> {
        Some(rebuild(self.nature?, coefficient))
    }
}

/// Comparisons retain the selected operand, including its percentage type.
pub(super) struct Comparison {
    common: Homogeneous,
    kind: u8,
    count: usize,
    selected: Option<(Val, f32)>,
    clamp: [(Val, f32); 3],
    nan: bool,
}

impl Comparison {
    pub(super) fn new(kind: u8) -> Self {
        Self {
            common: Homogeneous::default(),
            kind,
            count: 0,
            selected: None,
            clamp: [(Val::Num(0.0), 0.0); 3],
            nan: false,
        }
    }

    pub(super) fn push(&mut self, value: Val) -> Option<()> {
        let key = self.common.push(value)?;
        self.nan |= key.is_nan();
        if self.kind == 2 {
            *self.clamp.get_mut(self.count)? = (value, key);
        } else if self.selected.is_none_or(|(_, previous)| {
            if self.kind == 0 {
                key < previous
            } else {
                key > previous
            }
        }) {
            self.selected = Some((value, key));
        }
        self.count += 1;
        Some(())
    }

    pub(super) fn finish(self) -> Option<Val> {
        if self.kind == 2 && self.count != 3 {
            return None;
        }
        if self.nan {
            return self.common.rebuild(f32::NAN);
        }
        if self.kind == 2 {
            let want = self.clamp[1].1.min(self.clamp[2].1).max(self.clamp[0].1);
            self.clamp
                .into_iter()
                .find(|(_, k)| *k == want)
                .map(|(v, _)| v)
        } else {
            self.selected.map(|(v, _)| v)
        }
    }
}
