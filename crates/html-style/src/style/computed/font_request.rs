//! Преобразование вычисленных входов в нейтральный запрос matching.

use super::Computed;
use {
    kamin_font_facts::font_facts::FontOrientation, kamin_font_facts::font_facts::FontRequest,
    kamin_font_facts::font_facts::FontStyle,
};

impl Computed {
    pub fn font_request<'a>(&'a self, family_list: &'a str, size_px: f32) -> FontRequest<'a> {
        let wm = self.wm();
        FontRequest {
            weight: self.inh.font.font_weight.unwrap_or(400) as f32,
            width: self.inh.font.font_stretch.unwrap_or(100.0),
            style: if self.inh.font.oblique == Some(true) {
                FontStyle::Oblique(self.inh.font.font_oblique_angle.unwrap_or(14.0))
            } else if self.inh.font.italic == Some(true) {
                FontStyle::Italic
            } else {
                FontStyle::Normal
            },
            oblique_only: self.inh.font.font_oblique_only == Some(true),
            monospace: self.inh.font.monospace == Some(true),
            language: self.inh.text_flow.lang.as_deref(),
            features: &self.inh.font.font_features,
            feature_settings: self.inh.font.font_settings.as_deref().unwrap_or(&[]),
            synthesis: (
                self.inh.font.font_synth.0 != Some(false),
                self.inh.font.font_synth.1 != Some(false),
                self.inh.font.font_synth.2 != Some(false),
                self.inh.font.font_synth_position != Some(false),
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
