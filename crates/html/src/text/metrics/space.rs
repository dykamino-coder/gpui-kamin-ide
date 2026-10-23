//! Ширина пробела U+0020 через шейпер текста, с подстановкой глифов и возможностями шрифта:
//! числовой `tab-size` (css-text-3) считается от продвижения пробела блочного контейнера, а
//! глиф нуля в пропорциональных и подстановочных шрифтах ему не равен. Щуп ставит владелец
//! системы шрифтов; без него или при негодном замере — `None`.

use crate::document::state::ValueCx;
use gpui::{Font, TextSystem, px};
use std::sync::Arc;

pub(super) type SpaceProbe = Box<dyn Fn(&Font, f32) -> f32>;

pub fn install_space_probe(probe: impl Fn(&Font, f32) -> f32 + 'static, vcx: ValueCx<'_>) {
    *vcx.doc.metrics.space.borrow_mut() = Some(Box::new(probe));
    vcx.doc.metrics.invalidate();
}

pub fn space_advance(font: &Font, size: f32, vcx: ValueCx<'_>) -> Option<f32> {
    vcx.doc
        .metrics
        .space
        .borrow()
        .as_ref()
        .map(|probe| probe(font, size))
        .filter(|v| v.is_finite() && *v >= 0.0)
}

pub(super) fn use_text_system(text_system: Arc<TextSystem>, vcx: ValueCx<'_>) {
    install_space_probe(
        move |font, size| f32::from(text_system.measure_line(" ", font, px(size))),
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
            vcx,
        );
        let mut font = gpui::font("SpaceFont");
        font.weight = gpui::FontWeight(700.0);
        assert_eq!(space_advance(&font, 40.0, vcx), Some(10.0));
        assert_eq!(space_advance(&font, 0.0, vcx), Some(0.0));
    }

    #[test]
    fn missing_or_invalid_measurements_are_explicit() {
        let doc = crate::document::state::DocState::new(Default::default());
        let vcx = doc.values();
        let font = gpui::font("SpaceFont");
        assert_eq!(space_advance(&font, 20.0, vcx), None);
        install_space_probe(|_, _| f32::NAN, vcx);
        assert_eq!(space_advance(&font, 20.0, vcx), None);
        install_space_probe(|_, _| -1.0, vcx);
        assert_eq!(space_advance(&font, 20.0, vcx), None);
    }
}
