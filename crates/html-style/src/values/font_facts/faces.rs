//! Реестр `@font-face` одного документа: CSS-имена, дескрипторы и загруженные источники.
//! Читают его загрузчик и подбор лиц text; неизменяемый снимок — `font_facts::Registry`.

use std::{cell::RefCell, collections::HashMap};

#[derive(Default)]
pub struct FontFaces {
    pub aliases: RefCell<HashMap<String, String>>,
    pub features: RefCell<HashMap<String, Vec<(String, u32)>>>,
    pub faces: RefCell<HashMap<String, Vec<kamin_font_facts::font_facts::matching::Face>>>,
    pub has_space: RefCell<HashMap<String, bool>>,
    pub size_adjust: RefCell<HashMap<String, Vec<(u8, f32)>>>,
    pub loaded: RefCell<HashMap<String, Option<String>>>,
    pub romn_by_src: RefCell<HashMap<String, Option<f32>>>,
    pub romn: RefCell<HashMap<String, f32>>,
}

impl FontFaces {
    pub fn snapshot(
        &self,
        installed: std::rc::Rc<std::collections::HashSet<String>>,
        default_family: String,
        alternates: kamin_font_facts::font_facts::FeatureDefinitions,
        metric_overrides: kamin_font_facts::font_facts::MetricFaces,
    ) -> kamin_font_facts::font_facts::Registry {
        kamin_font_facts::font_facts::Registry {
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
