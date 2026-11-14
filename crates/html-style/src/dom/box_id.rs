//! Идентичность коробки в арене документа, отдельно от идентичности исходного DOM.

use std::cell::Cell;

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

/// Привязка входного представления к коробке; не часть значений документа.
#[derive(Clone)]
pub struct BoxLink(Cell<BoxId>);

impl BoxLink {
    pub fn get(&self) -> BoxId {
        self.0.get()
    }

    pub fn set(&self, id: BoxId) {
        self.0.set(id);
    }
}

impl Default for BoxLink {
    fn default() -> Self {
        Self(Cell::new(BoxId::NONE))
    }
}

impl PartialEq for BoxLink {
    fn eq(&self, _: &Self) -> bool {
        // Как TextId: идентичность хранения не меняет семантику дерева.
        true
    }
}

impl std::fmt::Debug for BoxLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BoxLink")
    }
}
