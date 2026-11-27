//! Keys of the bounded caches: exact float fields and keys hashed by a hint.

use std::hash::{Hash, Hasher};

/// A float in a key: finite values are kept exactly, both zeros are equal
/// and every NaN is one value; infinities differ by sign. A raw `f32` cannot
/// be `Eq`: NaN is not equal to itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FloatKey(u32);

impl FloatKey {
    pub fn get(self) -> f32 {
        f32::from_bits(self.0)
    }
}

impl From<f32> for FloatKey {
    fn from(value: f32) -> Self {
        Self(if value == 0.0 {
            0
        } else if value.is_nan() {
            f32::NAN.to_bits()
        } else {
            value.to_bits()
        })
    }
}

/// A key without its own `Hash`: hashed by `hint`, compared whole. The hint
/// takes part in equality; it is built from the key's canonical fields.
#[derive(Clone, PartialEq, Eq)]
pub struct Hinted<K> {
    pub hint: u64,
    pub key: K,
}

impl<K> Hash for Hinted<K> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hint.hash(state);
    }
}

/// The hint of the hashable part of a key.
pub fn hint(part: impl Hash) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    part.hash(&mut hasher);
    hasher.finish()
}
