//! Ширина пробела U+0020 через шейпер текста, с подстановкой глифов и возможностями шрифта:
//! числовой `tab-size` (css-text-3) считается от продвижения пробела блочного контейнера, а
//! глиф нуля в пропорциональных и подстановочных шрифтах ему не равен. Щуп ставит владелец
//! системы шрифтов; без него или при негодном замере — `None`.

use crate::document::state::ValueCx;
use gpui::{Font, TextSystem, px};
use std::sync::Arc;

pub(super) type SpaceProbe = Box<dyn Fn(&Font, f32) -> f32>;

pub(crate) fn install_space_probe(
    probe: impl Fn(&Font, f32) -> f32 + 'static,
    fonts: &crate::text::text_fonts::TextFonts,
    vcx: ValueCx<'_>,
) {
    *fonts.metrics.space.borrow_mut() = Some(Box::new(probe));
    vcx.doc.font_inputs_changed();
}

pub(crate) fn space_advance(
    font: &Font,
    size: f32,
    fonts: &crate::text::text_fonts::TextFonts,
) -> Option<f32> {
    fonts
        .metrics
        .space
        .borrow()
        .as_ref()
        .map(|probe| probe(font, size))
        .filter(|v| v.is_finite() && *v >= 0.0)
}

pub(super) fn use_text_system(
    text_system: Arc<TextSystem>,
    fonts: &crate::text::text_fonts::TextFonts,
    vcx: ValueCx<'_>,
) {
    install_space_probe(
        move |font, size| f32::from(text_system.measure_line(" ", font, px(size))),
        fonts,
        vcx,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_preserves_the_complete_font_and_allows_zero_advance() {
        let doc = crate::document::state::DocState::new(Default::default());
        let vcx = doc.values();
        install_space_probe(
            |font, size| {
                assert_eq!(font.family.as_ref(), "SpaceFont");
                assert_eq!(font.weight, gpui::FontWeight(700.0));
                size * 0.25
            },
            &doc.text,
            vcx,
        );
        let mut font = gpui::font("SpaceFont");
        font.weight = gpui::FontWeight(700.0);
        assert_eq!(space_advance(&font, 40.0, &doc.text), Some(10.0));
        assert_eq!(space_advance(&font, 0.0, &doc.text), Some(0.0));
    }

    #[test]
    fn missing_or_invalid_measurements_are_explicit() {
        let doc = crate::document::state::DocState::new(Default::default());
        let vcx = doc.values();
        let font = gpui::font("SpaceFont");
        assert_eq!(space_advance(&font, 20.0, &doc.text), None);
        install_space_probe(|_, _| f32::NAN, &doc.text, vcx);
        assert_eq!(space_advance(&font, 20.0, &doc.text), None);
        install_space_probe(|_, _| -1.0, &doc.text, vcx);
        assert_eq!(space_advance(&font, 20.0, &doc.text), None);
    }
}
