//! Неизменяемый снимок входов matching документа. Здесь нет загрузчика или glyph fallback.

use super::{FontEnvironmentId, FontRequest, matching};
use std::collections::{HashMap, HashSet};
use std::{rc::Rc, sync::Arc};

pub(crate) type FeatureDefinitions = HashMap<(String, String, String), (Vec<u32>, Vec<u32>)>;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Registry {
    pub default_family: String,
    pub installed: Rc<HashSet<String>>,
    pub aliases: HashMap<String, String>,
    pub features: HashMap<String, Vec<(String, u32)>>,
    pub faces: HashMap<String, Vec<matching::Face>>,
    pub has_space: HashMap<String, bool>,
    pub alternates: Arc<FeatureDefinitions>,
    pub size_adjust: HashMap<String, Vec<(u8, f32)>>,
    pub metric_overrides: super::MetricFaces,
}

/// Данные удерживаются снимком; новая регистрация не меняет старый снимок.
#[derive(Clone, Debug, PartialEq)]
pub struct FontRegistrySnapshot {
    pub(crate) environment: FontEnvironmentId,
    pub(crate) generation: u64,
    pub(crate) registry: Rc<Registry>,
}

impl FontRegistrySnapshot {
    pub fn environment(&self) -> &FontEnvironmentId {
        &self.environment
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn default_family(&self) -> &str {
        &self.registry.default_family
    }

    pub fn installed(&self, family: &str) -> bool {
        self.registry
            .installed
            .contains(&family.to_ascii_lowercase())
    }

    pub fn fonts_known(&self) -> bool {
        !self.registry.installed.is_empty()
    }

    pub fn alias(&self, family: &str) -> Option<String> {
        self.registry
            .aliases
            .get(&family.to_ascii_lowercase())
            .cloned()
    }

    /// U+0020 проверяется по unicode-range, без запроса наличия глифа в backend.
    /// Сохраняется прежнее объединение диапазонов на уровне семейства (M).
    pub fn covers_space(&self, family: &str) -> bool {
        self.registry
            .has_space
            .get(&family.to_ascii_lowercase())
            .copied()
            .unwrap_or(true)
    }

    pub fn first_available_named<'a>(
        &self,
        families: impl IntoIterator<Item = &'a str>,
    ) -> Option<&'a str> {
        families.into_iter().find(|family| {
            (self.installed(family) || self.alias(family).is_some()) && self.covers_space(family)
        })
    }

    pub fn mono_family(&self, language: Option<&str>) -> &'static str {
        let ja = language.is_some_and(|l| {
            let l = l.trim().to_ascii_lowercase();
            l == "ja" || l.starts_with("ja-")
        });
        if ja && self.installed("MS Gothic") {
            return "MS Gothic";
        }
        ["JetBrains Mono", "Consolas", "Courier New"]
            .into_iter()
            .find(|name| self.installed(name))
            .unwrap_or("JetBrains Mono")
    }

    pub(crate) fn matching_face(
        &self,
        family: &str,
        request: FontRequest<'_>,
    ) -> Option<matching::Face> {
        matching::select(
            self.registry.faces.get(&family.to_ascii_lowercase())?,
            &request,
        )
    }

    pub(crate) fn alias_for(&self, family: &str, request: FontRequest<'_>) -> Option<String> {
        self.matching_face(family, request)
            .map(|f| f.family)
            .or_else(|| self.alias(family))
    }

    pub(crate) fn face_features(
        &self,
        family: &str,
        request: FontRequest<'_>,
    ) -> Vec<(String, u32)> {
        self.matching_face(family, request)
            .map(|face| matching::features(face, &request))
            .unwrap_or_else(|| {
                self.registry
                    .features
                    .get(&family.to_ascii_lowercase())
                    .cloned()
                    .unwrap_or_default()
            })
    }

    pub(crate) fn alternates(&self) -> Arc<FeatureDefinitions> {
        self.registry.alternates.clone()
    }
}

/// Семейство за родовое `sans-serif`; оно же — умолчание документа.
pub const GENERIC_SANS: &str = "Segoe UI";

pub fn generic_family(lower: &str) -> Option<&'static str> {
    match lower {
        "system-ui" | "sans-serif" | "ui-sans-serif" | "ui-rounded" => Some(GENERIC_SANS),
        "serif" | "ui-serif" => Some("Times New Roman"),
        "cursive" => Some("Comic Sans MS"),
        "fantasy" => Some("Impact"),
        _ => None,
    }
}
