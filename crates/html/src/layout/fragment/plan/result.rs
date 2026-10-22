//! Результат планирования: геометрия и явная неполнота при исчерпании копий.

use crate::layout::fragment::types::Frag;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Completion {
    #[default]
    Complete,
    Incomplete,
}

/// Неполнота монотонна: добавление фрагментов не восстанавливает потерянный хвост.
#[derive(Clone, Default)]
pub(crate) struct Plan {
    pub(crate) fragments: Vec<Frag>,
    pub(crate) completion: Completion,
}

impl Plan {
    pub(crate) fn incomplete(&mut self) {
        self.completion = Completion::Incomplete;
    }

    /// Проверка построенных копий до prepaint; отсутствующий фрагмент остаётся в плане.
    pub(crate) fn check_copies(&mut self, available: impl Fn(usize) -> usize) {
        if self.iter().any(|f| f.copy >= available(f.kid)) {
            self.incomplete();
        }
    }
}

impl std::ops::Deref for Plan {
    type Target = Vec<Frag>;
    fn deref(&self) -> &Self::Target {
        &self.fragments
    }
}

impl std::ops::DerefMut for Plan {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.fragments
    }
}

impl IntoIterator for Plan {
    type Item = Frag;
    type IntoIter = std::vec::IntoIter<Frag>;
    fn into_iter(self) -> Self::IntoIter {
        self.fragments.into_iter()
    }
}

impl<'a> IntoIterator for &'a Plan {
    type Item = &'a Frag;
    type IntoIter = std::slice::Iter<'a, Frag>;
    fn into_iter(self) -> Self::IntoIter {
        self.fragments.iter()
    }
}
