//! Box identity in the document arena, apart from the identity of the source DOM.

/// Arena slot key; the generation keeps a removed box from passing for a new one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct BoxId {
    pub slot: u32,
    pub generation: u32,
}

impl BoxId {
    /// Generation zero is reserved for a missing link.
    pub const NONE: Self = Self {
        slot: 0,
        generation: 0,
    };

    pub fn is_some(self) -> bool {
        self.generation != 0
    }
}
