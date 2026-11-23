//! The document sequence participates in invalidation without an element style.
use super::{Children, OutputOwnership};

#[derive(Clone, Default)]
pub struct RootOutput {
    pub raw: Children,
    pub ownership: OutputOwnership,
}
