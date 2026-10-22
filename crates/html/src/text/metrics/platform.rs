//! Явная конфигурация платформенных шрифтов: разделяемые ресурсы без состояния документа.

use crate::document::state::ValueCx;
use std::{collections::HashSet, rc::Rc, sync::Arc};

type FontLoader = Rc<dyn Fn(Vec<u8>) -> Option<String>>;

/// Источник шрифтов для разбора документа и его вложенных документов.
/// Копия разделяет платформенные ресурсы, но не щупы и кэши измерений.
#[derive(Clone, Default)]
pub struct FontContext {
    pub(super) system: Option<Arc<gpui::TextSystem>>,
    pub(super) loader: Option<FontLoader>,
    pub(super) installed: Rc<HashSet<String>>,
}

impl FontContext {
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

    pub(crate) fn install(&self, vcx: ValueCx<'_>) {
        vcx.doc.metrics.platform.replace(self.clone());
        *vcx.doc.metrics.wrap.borrow_mut() = None;
        *vcx.doc.metrics.space.borrow_mut() = None;
        if let Some(system) = &self.system {
            super::text_system::bind_measurements(system.clone(), vcx);
        }
        vcx.doc.metrics.cache.borrow_mut().clear();
        vcx.doc.metrics.vcache.borrow_mut().clear();
        vcx.doc.metrics.parts.borrow_mut().clear();
    }
}
