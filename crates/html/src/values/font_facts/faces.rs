//! Реестр `@font-face` одного документа: CSS-имена, дескрипторы и загруженные источники.
//! Читают его загрузчик и подбор лиц text; неизменяемый снимок — `font_facts::Registry`.

use std::{cell::RefCell, collections::HashMap};

#[derive(Default)]
pub(crate) struct FontFaces {
    pub(crate) aliases: RefCell<HashMap<String, String>>,
    pub(crate) features: RefCell<HashMap<String, Vec<(String, u32)>>>,
    pub(crate) faces: RefCell<HashMap<String, Vec<crate::font_facts::matching::Face>>>,
    pub(crate) has_space: RefCell<HashMap<String, bool>>,
    pub(crate) size_adjust: RefCell<HashMap<String, Vec<(u8, f32)>>>,
    pub(crate) loaded: RefCell<HashMap<String, Option<String>>>,
    pub(crate) romn_by_src: RefCell<HashMap<String, Option<f32>>>,
    pub(crate) romn: RefCell<HashMap<String, f32>>,
}

impl FontFaces {
    pub(crate) fn snapshot(
        &self,
        installed: std::rc::Rc<std::collections::HashSet<String>>,
        default_family: String,
        alternates: crate::font_facts::FeatureDefinitions,
        metric_overrides: crate::font_facts::MetricFaces,
    ) -> crate::font_facts::Registry {
        crate::font_facts::Registry {
            size_adjust: self.size_adjust.borrow().clone(),
            metric_overrides,
            installed,
            default_family,
            aliases: self.aliases.borrow().clone(),
            features: self.features.borrow().clone(),
            faces: self.faces.borrow().clone(),
            has_space: self.has_space.borrow().clone(),
            alternates: std::sync::Arc::new(alternates),
        }
    }
}
