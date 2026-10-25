//! CSS-имена font-face, дескрипторы и загруженные источники, принадлежащие одному документу.

use std::{cell::RefCell, collections::HashMap};

#[derive(Default)]
pub(crate) struct FontFaces {
    pub(super) aliases: RefCell<HashMap<String, String>>,
    pub(super) features: RefCell<HashMap<String, Vec<(String, u32)>>>,
    pub(super) faces: RefCell<HashMap<String, Vec<super::matching::Face>>>,
    pub(super) has_space: RefCell<HashMap<String, bool>>,
    pub(super) size_adjust: RefCell<HashMap<String, Vec<(u8, f32)>>>,
    pub(super) loaded: RefCell<HashMap<String, Option<String>>>,
    pub(super) romn_by_src: RefCell<HashMap<String, Option<f32>>>,
    pub(super) romn: RefCell<HashMap<String, f32>>,
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
