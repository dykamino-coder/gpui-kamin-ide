//! Адаптация нативного выбора: identity приходит от FontId, не от имени семейства.

use super::Backend;
use crate::font_facts::{
    FaceId, FontFactsError, FontInstanceFacts, FontRegistrySnapshot, FontRequest, FontStyle,
    SelectedFaceFacts,
};

impl Backend {
    pub(super) fn select_face(
        &self,
        request: FontRequest<'_>,
        snapshot: &FontRegistrySnapshot,
    ) -> Result<SelectedFaceFacts, FontFactsError> {
        let doc = self.document(snapshot)?;
        if request.family_list.contains(',') {
            return Err(FontFactsError::UnsupportedRequest);
        }
        // Выбор нативного лица сохраняется; неподдерживаемые instance-входы
        // отражены в фактах, а не превращены в новый fallback рендеринга.
        let instance = if !request.variations.is_empty()
            || request.optical_size.is_some()
            || request.script.is_some()
        {
            FontInstanceFacts::Unsupported {
                variations: !request.variations.is_empty(),
                optical_size: request.optical_size.is_some(),
                script: request.script.is_some(),
            }
        } else {
            FontInstanceFacts::Absent
        };
        let descriptor = snapshot.matching_face(request.family_list, request);
        let font = font(request, snapshot);
        let (backend_key, units_per_em) = doc
            .metrics
            .font_context()
            .native_face(&font)
            .ok_or(FontFactsError::Unavailable)?;
        Ok(SelectedFaceFacts {
            face: FaceId {
                environment: snapshot.environment.clone(),
                generation: snapshot.generation,
                backend_key,
            },
            units_per_em,
            size_adjust: descriptor.map_or(1.0, |face| face.adjust),
            instance,
        })
    }
}

pub(super) fn font(request: FontRequest<'_>, snapshot: &FontRegistrySnapshot) -> gpui::Font {
    let family = request.family_list;
    let resolved = snapshot.alias_for(family, request);
    let family = resolved.as_deref().unwrap_or(family);
    let family = if family.is_empty() && request.monospace {
        snapshot.mono_family(request.language)
    } else if family.is_empty() && !snapshot.default_family().is_empty() {
        snapshot.default_family()
    } else if family.is_empty() {
        crate::font_facts::GENERIC_SANS
    } else {
        family
    };
    let mut font = gpui::font(family.to_string());
    font.weight = gpui::FontWeight(request.weight);
    font.stretch = gpui::FontStretch::from_percent(request.width);
    font.style = match request.style {
        FontStyle::Normal => gpui::FontStyle::Normal,
        FontStyle::Italic | FontStyle::Oblique(_) => gpui::FontStyle::Italic,
    };
    let mut features = snapshot.face_features(request.family_list, request);
    features.extend_from_slice(request.features);
    features.extend_from_slice(request.feature_settings);
    features.push((crate::font_facts::CSS_WEIGHT_TAG.0.into(), 1));
    for (tag, allow) in [("nsyw", request.synthesis.0), ("nsys", request.synthesis.1)] {
        if !allow {
            features.push((tag.into(), 1));
        }
    }
    font.features = gpui::FontFeatures(std::sync::Arc::new(features));
    font
}
