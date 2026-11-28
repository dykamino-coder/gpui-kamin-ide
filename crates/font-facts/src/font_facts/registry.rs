//! Неизменяемый снимок входов matching документа. Здесь нет загрузчика или glyph fallback.

use super::{FontEnvironmentId, FontRequest, key, matching};
use std::collections::{HashMap, HashSet};
use std::{rc::Rc, sync::Arc};

pub type FeatureDefinitions = HashMap<(String, String, String), (Vec<u32>, Vec<u32>)>;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Registry {
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
    pub environment: FontEnvironmentId,
    pub generation: u64,
    pub registry: Rc<Registry>,
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
        key::with(family, |family| self.registry.installed.contains(family))
    }

    pub fn fonts_known(&self) -> bool {
        !self.registry.installed.is_empty()
    }

    pub fn alias(&self, family: &str) -> Option<String> {
        key::with(family, |family| self.registry.aliases.get(family).cloned())
    }

    /// U+0020 проверяется по unicode-range, без запроса наличия глифа в backend.
    /// Сохраняется прежнее объединение диапазонов на уровне семейства (M).
    pub fn covers_space(&self, family: &str) -> bool {
        key::with(family, |family| {
            self.registry.has_space.get(family).copied()
        })
        .unwrap_or(true)
    }

    pub fn first_available_named<'a>(
        &self,
        families: impl IntoIterator<Item = &'a str>,
    ) -> Option<&'a str> {
        families.into_iter().find_map(|family| {
            if self.installed(family) || self.alias(family).is_some() {
                return self.covers_space(family).then_some(family);
            }
            // CSS Fonts 4 section 5.1 permits platform family substitutions.
            // Try a conventional equivalent only after the authored family,
            // including document faces, has failed to match.
            let alternate = [
                ("Helvetica", "Arial"),
                ("Arial", "Helvetica"),
                ("Times", "Times New Roman"),
                ("Times New Roman", "Times"),
                ("Courier", "Courier New"),
                #[cfg(not(target_os = "windows"))]
                ("Courier New", "Courier"),
            ]
            .into_iter()
            .find_map(|(name, equivalent)| {
                family.eq_ignore_ascii_case(name).then_some(equivalent)
            })?;
            self.installed(alternate).then_some(alternate)
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

    pub fn matching_face(&self, family: &str, request: FontRequest<'_>) -> Option<matching::Face> {
        let faces = key::with(family, |family| self.registry.faces.get(family))?;
        matching::select(faces, &request)
    }

    /// Every face of `family`'s composite for `request` as (backend family,
    /// `unicode-range`), the leading face first (`coverage::composite`).
    pub fn composite_for(
        &self,
        family: &str,
        request: FontRequest<'_>,
    ) -> Vec<(String, Vec<(u32, u32)>)> {
        key::with(family, |family| self.registry.faces.get(family))
            .map(|faces| {
                super::coverage::composite(faces, &request)
                    .into_iter()
                    .map(|face| (face.family.clone(), face.ranges.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn alias_for(&self, family: &str, request: FontRequest<'_>) -> Option<String> {
        self.matching_face(family, request)
            .map(|f| f.family)
            .or_else(|| self.alias(family))
    }

    pub fn face_features(&self, family: &str, request: FontRequest<'_>) -> Vec<(String, u32)> {
        self.matching_face(family, request)
            .map(|face| matching::features(face, &request))
            .unwrap_or_else(|| {
                key::with(family, |family| self.registry.features.get(family).cloned())
                    .unwrap_or_default()
            })
    }

    pub fn alternates(&self) -> Arc<FeatureDefinitions> {
        self.registry.alternates.clone()
    }
}

/// The family behind the generic `sans-serif` on this platform.
pub const GENERIC_SANS: &str = "Arial";

/// The platform's generic family table (css-fonts-4 section 4.2.1), as Chrome on
/// Windows resolves it: its default font settings name Times New Roman, Arial,
/// Comic Sans MS and Impact, and `system-ui` is the system message font.
/// `monospace` is resolved later by language (`mono_family`). `ui-serif`,
/// `ui-sans-serif`, `ui-monospace` and `ui-rounded` have no face on Windows:
/// such a generic "does not match any font", so the list walk goes on to the
/// next family instead of borrowing a neighbour's face.
pub fn generic_family(lower: &str) -> Option<&'static str> {
    match lower {
        "sans-serif" => Some(GENERIC_SANS),
        "system-ui" => Some("Segoe UI"),
        "serif" => Some("Times New Roman"),
        "cursive" => Some("Comic Sans MS"),
        "fantasy" => Some("Impact"),
        _ => None,
    }
}
