//! Идентичность коробки в арене документа, отдельно от идентичности исходного DOM.

/// Ключ слота арены; поколение не позволяет принять удалённую коробку за новую.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct BoxId {
    pub slot: u32,
    pub generation: u32,
}

impl BoxId {
    /// Поколение ноль зарезервировано для отсутствующей связи.
    pub const NONE: Self = Self {
        slot: 0,
        generation: 0,
    };

    pub fn is_some(self) -> bool {
        self.generation != 0
    }
}
