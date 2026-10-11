//! Measure U+0020 through the text shaper, including glyph fallback and font features.
//!
//! CSS Text numeric tab sizes use the block container's space advance. The
//! zero glyph is not an equivalent metric in proportional or fallback fonts.

use gpui::{Font, TextSystem, px};
use std::{cell::RefCell, sync::Arc};

type SpaceProbe = Box<dyn Fn(&Font, f32) -> f32>;

thread_local! {
    static PROBE: RefCell<Option<SpaceProbe>> = const { RefCell::new(None) };
}

pub fn install_space_probe(probe: impl Fn(&Font, f32) -> f32 + 'static) {
    PROBE.with(|p| *p.borrow_mut() = Some(Box::new(probe)));
}

pub fn space_advance(font: &Font, size: f32) -> Option<f32> {
    PROBE
        .with(|p| p.borrow().as_ref().map(|probe| probe(font, size)))
        .filter(|v| v.is_finite() && *v >= 0.0)
}

pub(super) fn use_text_system(text_system: Arc<TextSystem>) {
    install_space_probe(move |font, size| f32::from(text_system.measure_line(" ", font, px(size))));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_preserves_the_complete_font_and_allows_zero_advance() {
        install_space_probe(|font, size| {
            assert_eq!(font.family.as_ref(), "SpaceFont");
            assert_eq!(font.weight, gpui::FontWeight(700.0));
            size * 0.25
        });
        let mut font = gpui::font("SpaceFont");
        font.weight = gpui::FontWeight(700.0);
        assert_eq!(space_advance(&font, 40.0), Some(10.0));
        assert_eq!(space_advance(&font, 0.0), Some(0.0));
    }

    #[test]
    fn missing_or_invalid_measurements_are_explicit() {
        let font = gpui::font("SpaceFont");
        assert_eq!(space_advance(&font, 20.0), None);
        install_space_probe(|_, _| f32::NAN);
        assert_eq!(space_advance(&font, 20.0), None);
        install_space_probe(|_, _| -1.0);
        assert_eq!(space_advance(&font, 20.0), None);
    }
}
