//! Преобразование вычисленных входов в нейтральный запрос matching.

use super::Computed;
use crate::font_facts::{FontOrientation, FontRequest, FontStyle};

impl Computed {
    pub(crate) fn font_request<'a>(
        &'a self,
        family_list: &'a str,
        size_px: f32,
    ) -> FontRequest<'a> {
        let wm = self.wm();
        FontRequest {
            weight: self.font_weight.unwrap_or(400) as f32,
            width: self.font_stretch.unwrap_or(100.0),
            style: if self.oblique == Some(true) {
                FontStyle::Oblique(self.font_oblique_angle.unwrap_or(14.0))
            } else if self.italic == Some(true) {
                FontStyle::Italic
            } else {
                FontStyle::Normal
            },
            oblique_only: self.font_oblique_only == Some(true),
            monospace: self.monospace == Some(true),
            language: self.lang.as_deref(),
            features: &self.font_features,
            feature_settings: self.font_settings.as_deref().unwrap_or(&[]),
            synthesis: (
                self.font_synth.0 != Some(false),
                self.font_synth.1 != Some(false),
                self.font_synth.2 != Some(false),
                self.font_synth_position != Some(false),
            ),
            orientation: if wm.sideways() {
                FontOrientation::Sideways
            } else if !wm.vertical() {
                FontOrientation::Horizontal
            } else if wm.upright() {
                FontOrientation::VerticalUpright
            } else {
                FontOrientation::VerticalMixed
            },
            ..FontRequest::new(family_list, size_px)
        }
    }
}
