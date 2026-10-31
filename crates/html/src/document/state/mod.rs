//! Состояние документа целиком (`DocState`): нижние данные значений
//! (`crate::values`) и состояния фаз раскладки и текста этого документа, плюс
//! обёртка поддерева, отмечающая документ покрашенным (`Scoped`).

mod phases;
mod scoped;

pub use phases::DocState;
pub use scoped::Scoped;

/// Шрифтовые факты документа с бэкендом text: адаптер измерений и читатели CSS.
#[cfg(test)]
mod font_facts {
    use crate::font_facts::{FontFactsProvider, FontRegistrySnapshot};
    use std::rc::Rc;

    mod readers;
    mod tests;
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod property_registry;

#[cfg(test)]
mod readiness;

#[cfg(test)]
mod synthetic_ids;

#[cfg(test)]
mod traversal;
