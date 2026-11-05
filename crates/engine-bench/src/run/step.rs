//! Контракт шага: локальная правка тоже может выполнять parse/style/document work.

/// Что меняется перед кадром замера.
#[derive(Clone, Copy)]
pub(crate) enum Step<'a> {
    /// Новый документ: из файла входа или из данной разметки.
    Cold(Option<&'a str>),
    /// Тот же документ, принудительная перерисовка.
    Warm,
    /// `Document::update` данной разметкой (локальная правка).
    Update(&'a str),
}

impl Step<'_> {
    pub(super) fn document_work(self) -> bool {
        !matches!(self, Self::Warm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_keeps_document_phase_measurements_while_warm_skips_them() {
        assert!(Step::Cold(None).document_work());
        assert!(Step::Update("<p>changed").document_work());
        assert!(!Step::Warm.document_work());
    }
}
