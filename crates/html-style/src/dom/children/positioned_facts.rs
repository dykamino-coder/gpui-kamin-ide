//! Lazily answered questions about positioned content in the subtrees of a
//! list, owned by the copy-on-write children snapshot.
//!
//! The answers depend only on the nodes of the list and their subtrees: any
//! change of a node, its style or a descendant reaches the list through
//! `DerefMut` (or `dirty_intrinsic_sizes`) and forgets them, a cloned
//! snapshot starts empty.

/// One question about the subtrees of a list; its meaning is defined by the
/// layout predicates that ask it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PositionedQuery {
    /// Positioned content that stays in place, outside a containing block.
    StaysOutsideCb = 0,
    /// Positioned content that stays in place, under a containing block.
    StaysUnderCb = 1,
    /// Any relative, sticky or absolute element.
    Positioned = 2,
}

/// Known answers: bit `q` of `known` marks that bit `q` of `value` is set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PositionedFacts {
    known: u8,
    value: u8,
}

impl PositionedFacts {
    pub fn get(self, query: PositionedQuery) -> Option<bool> {
        let bit = 1 << query as u8;
        (self.known & bit != 0).then_some(self.value & bit != 0)
    }

    pub fn with(self, query: PositionedQuery, answer: bool) -> Self {
        let bit = 1 << query as u8;
        Self {
            known: self.known | bit,
            value: if answer {
                self.value | bit
            } else {
                self.value & !bit
            },
        }
    }
}
