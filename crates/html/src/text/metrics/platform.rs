//! Явная конфигурация платформенных шрифтов: разделяемые ресурсы без состояния документа.

use crate::document::state::ValueCx;
use std::{collections::HashSet, rc::Rc, sync::Arc};

type FontLoader = Rc<dyn Fn(Vec<u8>) -> Option<String>>;
type FactsFactory = Rc<dyn Fn() -> Rc<dyn crate::font_facts::FontFactsProvider>>;

#[cfg(test)]
mod tests;

/// Источник шрифтов для разбора документа и его вложенных документов.
/// Копия разделяет платформенные ресурсы, но не щупы и кэши измерений.
#[derive(Clone, Default)]
pub struct FontContext {
    pub(super) system: Option<Arc<gpui::TextSystem>>,
    pub(super) loader: Option<FontLoader>,
    pub(super) installed: Rc<HashSet<String>>,
    pub(super) facts_factory: Option<FactsFactory>,
}

impl FontContext {
    pub(crate) fn native_face(&self, font: &gpui::Font) -> Option<(usize, u32)> {
        let system = self.system.as_ref()?;
        let id = system.resolve_font(font);
        Some((id.0, system.units_per_em(id)))
    }

    /// Метрики конкретного лица до size-adjust и metric overrides.
    pub(crate) fn native_metrics(
        &self,
        font: &gpui::Font,
        size: f32,
    ) -> Option<crate::font_facts::FontMetricFacts> {
        use crate::font_facts::{FontMetric, FontMetricFacts, MetricSource};
        let system = self.system.as_ref()?;
        let id = system.resolve_font(font);
        let px = gpui::px(size);
        let metric = |v: gpui::Pixels| {
            Some(FontMetric {
                px: v.into(),
                source: MetricSource::Native,
            })
        };
        let advance = |ch| {
            system
                .has_glyph(id, ch)
                .then(|| system.advance(id, px, ch).ok())
                .flatten()
                .and_then(|v| metric(v.width))
        };
        Some(FontMetricFacts {
            ascent: metric(system.ascent(id, px)),
            descent: metric(system.descent(id, px)),
            cap_height: metric(system.cap_height(id, px)),
            x_height: metric(system.x_height(id, px)),
            zero_advance: advance('0'),
            ideographic_advance: advance('水'),
            // TextSystem::advance возвращает только горизонтальное продвижение.
            // Отсутствие вертикального API сохраняем для CSS fallback, не как 0.
            ideographic_vertical_advance: None,
            ..FontMetricFacts::default()
        })
    }

    /// Снять список семейств после регистрации встроенных шрифтов приложения.
    pub fn new(system: Arc<gpui::TextSystem>) -> Self {
        let installed = Rc::new(
            system
                .all_font_names()
                .iter()
                .map(|n| n.to_ascii_lowercase())
                .collect(),
        );
        let fonts = system.clone();
        Self {
            system: Some(system),
            installed,
            ..Self::default()
        }
        .with_loader(move |bytes| {
            let named = crate::text::fonts::sfnt_family(&bytes);
            let before: HashSet<_> = fonts.all_font_names().into_iter().collect();
            fonts.add_fonts(vec![bytes.into()]).ok()?;
            named.or_else(|| {
                fonts
                    .all_font_names()
                    .into_iter()
                    .find(|n| !before.contains(n))
            })
        })
    }

    /// Задать загрузчик с дедупликацией нативных ресурсов в пределах его жизни.
    pub fn with_loader(mut self, loader: impl Fn(Vec<u8>) -> Option<String> + 'static) -> Self {
        self.loader = Some(Rc::from(super::font_loader::deduplicated(Box::new(loader))));
        self
    }

    /// Создать backend фактов отдельно для каждого документа, включая вложенные.
    /// Factory не получает дерево, style или GPUI; запрос несёт явный snapshot.
    pub fn with_facts_provider(
        mut self,
        factory: impl Fn() -> Rc<dyn crate::font_facts::FontFactsProvider> + 'static,
    ) -> Self {
        self.facts_factory = Some(Rc::new(factory));
        self
    }

    pub(crate) fn install(&self, fonts: &crate::text::text_fonts::TextFonts, vcx: ValueCx<'_>) {
        fonts.metrics.platform.replace(self.clone());
        vcx.doc.set_installed_fonts(self.installed.clone());
        *fonts.metrics.wrap.borrow_mut() = None;
        *fonts.metrics.space.borrow_mut() = None;
        if let Some(system) = &self.system {
            super::text_system::bind_measurements(system.clone(), fonts, vcx);
        }
        fonts.metrics.cache.borrow_mut().clear();
        fonts.metrics.vcache.borrow_mut().clear();
        fonts.metrics.parts.borrow_mut().clear();
        vcx.doc
            .install_font_facts(self.facts_factory.as_ref().map(|factory| factory()));
    }
}
