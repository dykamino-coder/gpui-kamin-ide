//! Владение provider и identity шрифтов одного документа; backend держит только Weak.

use super::DocState;
use crate::font_facts::{FontEnvironmentId, FontFactsProvider, FontRegistrySnapshot};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

#[derive(Default)]
pub(crate) struct DocumentFonts {
    environment: FontEnvironmentId,
    provider: RefCell<Option<Rc<dyn FontFactsProvider>>>,
    registry_generation: Cell<u64>,
    snapshot: RefCell<Option<FontRegistrySnapshot>>,
    /// Семейство документа: шрифт пустого `font-family` в замерах.
    family: RefCell<String>,
    /// Установленные семейства платформы, переданные `FontContext::install`.
    installed: RefCell<Rc<HashSet<String>>>,
    /// Растёт при каждой смене того, от чего зависят шрифтовые единицы
    /// вычисленного стиля (`ch`/`ex`/`lh`, `line-height: normal`): семейства
    /// документа, щупов и платформенного контекста шрифтов.
    inputs_generation: Cell<u64>,
}

impl DocState {
    /// Снимок переиспользуется до смены платформы, реестра или входов метрик.
    pub(crate) fn font_snapshot(&self) -> FontRegistrySnapshot {
        let generation = self.font_generation() + self.font_facts.registry_generation.get();
        if let Some(snapshot) = self.font_facts.snapshot.borrow().as_ref()
            && snapshot.generation == generation
        {
            return snapshot.clone();
        }
        let snapshot = FontRegistrySnapshot {
            environment: self.font_facts.environment.clone(),
            generation,
            registry: Rc::new(self.font_faces.snapshot(
                self.font_facts.installed.borrow().clone(),
                self.doc_family(),
                self.font_alternates.borrow().clone(),
                self.font_metric_faces.borrow().clone(),
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
            self.font_inputs_changed();
        }
    }

    /// Поколение шрифтовых входов вычисленного стиля (`style::cascade::materialize`).
    pub(crate) fn font_generation(&self) -> u64 {
        self.font_facts.inputs_generation.get()
    }

    /// Сменился вход шрифтовых единиц: семейство документа, щуп или платформа.
    pub(crate) fn font_inputs_changed(&self) {
        let generation = &self.font_facts.inputs_generation;
        generation.set(generation.get() + 1);
    }

    /// Семейство документа для пустого `font-family` в замерах.
    pub(crate) fn doc_family(&self) -> String {
        self.font_facts.family.borrow().clone()
    }

    /// Задать семейство документа; смена — новое поколение шрифтовых входов.
    pub(crate) fn set_doc_family(&self, family: &str) {
        if *self.font_facts.family.borrow() != family {
            self.font_facts.family.replace(family.to_owned());
            self.font_inputs_changed();
        }
    }

    /// Установленные семейства платформы этого документа.
    pub(crate) fn set_installed_fonts(&self, installed: Rc<HashSet<String>>) {
        self.font_facts.installed.replace(installed);
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

mod faces;
pub(crate) use faces::FontFaces;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod readers;
