//! Владение provider и identity шрифтов одного документа; backend держит только Weak.

use super::DocState;
use crate::font_facts::{FontEnvironmentId, FontFactsProvider, FontRegistrySnapshot};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Default)]
pub(crate) struct DocumentFonts {
    environment: FontEnvironmentId,
    provider: RefCell<Option<Rc<dyn FontFactsProvider>>>,
}

impl DocState {
    /// Поколение существующих кэшей остаётся единственным источником инвалидации.
    pub(crate) fn font_snapshot(&self) -> FontRegistrySnapshot {
        FontRegistrySnapshot {
            environment: self.font_facts.environment.clone(),
            generation: self.metrics.generation(),
        }
    }

    /// Настройка применяется только к этому документу, включая повторный bootstrap.
    pub(crate) fn install_font_facts(&self, provider: Option<Rc<dyn FontFactsProvider>>) {
        let previous =
            self.font_facts
                .provider
                .replace(Some(provider.unwrap_or_else(|| {
                    Rc::new(crate::text::font_facts::Backend::new(self.this.clone()))
                })));
        if previous.is_some() {
            self.metrics.invalidate();
        }
    }

    pub(crate) fn font_provider(&self) -> Rc<dyn FontFactsProvider> {
        self.font_facts
            .provider
            .borrow()
            .as_ref()
            .cloned()
            .unwrap_or_else(|| Rc::new(crate::text::font_facts::Backend::new(self.this.clone())))
    }
}

#[cfg(test)]
mod tests;
