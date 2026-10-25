//! Владение provider и identity шрифтов одного документа; backend держит только Weak.

use super::DocState;
use crate::font_facts::{FontEnvironmentId, FontFactsProvider, FontRegistrySnapshot};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(Default)]
pub(crate) struct DocumentFonts {
    environment: FontEnvironmentId,
    provider: RefCell<Option<Rc<dyn FontFactsProvider>>>,
    registry_generation: Cell<u64>,
    snapshot: RefCell<Option<FontRegistrySnapshot>>,
}

impl DocState {
    /// Снимок переиспользуется до смены платформы, реестра или входов метрик.
    pub(crate) fn font_snapshot(&self) -> FontRegistrySnapshot {
        let generation = self.metrics.generation() + self.font_facts.registry_generation.get();
        if let Some(snapshot) = self.font_facts.snapshot.borrow().as_ref()
            && snapshot.generation == generation
        {
            return snapshot.clone();
        }
        let snapshot = FontRegistrySnapshot {
            environment: self.font_facts.environment.clone(),
            generation,
            registry: Rc::new(self.font_faces.snapshot(
                self.metrics.font_context().installed_names(),
                self.metrics.default_family(),
                self.font_alternates.borrow().clone(),
            )),
        };
        self.font_facts.snapshot.replace(Some(snapshot.clone()));
        snapshot
    }

    /// Мутация загрузчика публикует новую ревизию, не меняя старые снимки.
    pub(crate) fn font_registry_changed(&self) {
        self.font_facts
            .registry_generation
            .set(self.font_facts.registry_generation.get() + 1);
    }

    pub(crate) fn reset_font_alternates(&self) {
        self.font_alternates.take();
        self.font_registry_changed();
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
