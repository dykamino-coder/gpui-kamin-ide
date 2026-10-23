//! Свёртка стопки одноцветных слоёв фона в изоляции с сохранением обрезок `text` и
//! `border-area`: для каждой области обрезки стопка сводится к одному цвету, который затем
//! красят глифы или рамка, а слои-картинки из стиля убираются.
//!
//! CSS Compositing 1 §3.4.3 смешивает слой с нижними слоями фона и цветом фона, но не со
//! страницей за элементом. Вычисление постоянной стопки по областям обрезки избавляет от
//! прямоугольной картинки поверх глифов; картинка, которую нельзя показать, считается `none`
//! (CSS Backgrounds 3 §2.3) и даёт прозрачный чёрный. Частичная альфа свёртке не подлежит.

use super::{BackgroundImageValues, ImageAvailability, flat_fill, over};
use crate::document::state::ValueCx;
use crate::style::computed::{BgClip, BlendMode, Computed};
use crate::style::values::value::Color;

mod blend;

pub(crate) fn collapse(c: &mut super::super::BoxStyle, vcx: ValueCx<'_>, images: &dyn BackgroundImageValues) {
    let Some(stack) = evaluate(c, vcx, images) else {
        return;
    };
    c.background = (stack[0].a > 0.0).then_some(stack[0]);
    c.flat_text_background = (stack[1] != stack[0] && stack[1].a == 1.0).then_some(stack[1]);
    c.flat_border_background = (stack[2] != stack[0] && stack[2].a == 1.0).then_some(stack[2]);
    c.bg_clip = None;
    c.bg_image = None;
    c.gradient = None;
    c.gradient_raw = None;
    c.bg_lists.clear();
    c.background_blend = None;
}

fn evaluate(c: &Computed, vcx: ValueCx<'_>, images: &dyn BackgroundImageValues) -> Option<[Color; 3]> {
    let layers = match c.bg_layers(vcx) {
        Some(layers) => layers,
        // Один слой — сам стиль: копия нужна только стопке, которую есть что сворачивать.
        None if !matches!(c.bg_clip, Some(BgClip::Text | BgClip::BorderArea))
            && !c.background_blend.is_some_and(|v| v != BlendMode::Normal) =>
        {
            return None;
        }
        None => {
            let mut image = c.clone();
            image.background = None;
            vec![image]
        }
    };
    let special = layers
        .iter()
        .any(|l| matches!(l.bg_clip, Some(BgClip::Text | BgClip::BorderArea)));
    let blended = layers
        .iter()
        .any(|l| l.background_blend.is_some_and(|v| v != BlendMode::Normal));
    if !special && !blended {
        return None;
    }
    let mut result = [Color::default(); 3];
    let background = c.background.unwrap_or_default();
    for (region, pixel) in result.iter_mut().enumerate() {
        if contains(c.color_clip(vcx), region)? {
            *pixel = background;
        }
    }
    for layer in layers.iter().rev() {
        let availability = layer
            .bg_image
            .as_ref()
            .map(|image| images.image_available(image, layer.image_orient_none != Some(true)));
        if availability == Some(ImageAvailability::Pending) {
            return None;
        }
        let unavailable = availability == Some(ImageAvailability::Failed);
        // CSS Backgrounds 3 §2.3: an image that cannot be displayed is
        // treated as none. It contributes transparent black to the stack,
        // without suppressing the lower background color or blend modes.
        let color = if unavailable || (layer.bg_image.is_none() && layer.gradient.is_none()) {
            Color::default()
        } else {
            flat_fill(layer, images)?
        };
        for (region, pixel) in result.iter_mut().enumerate() {
            if contains(layer.bg_clip, region)? {
                *pixel = blend::composite(
                    color,
                    *pixel,
                    layer.background_blend.unwrap_or(BlendMode::Normal),
                )?;
            }
        }
    }
    // A single glyph/border color is exact for an opaque constant region.
    // Partial alpha needs a mask composited separately from foreground ink.
    result
        .iter()
        .all(|p| p.a == 0.0 || p.a == 1.0)
        .then_some(result)
}

fn contains(clip: Option<BgClip>, region: usize) -> Option<bool> {
    Some(match clip {
        None | Some(BgClip::BorderBox) => true,
        Some(BgClip::Text) => region == 1,
        Some(BgClip::BorderArea) => region == 2,
        // Padding/content edges require separate geometry, not one box color.
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_blends_with_lower_backgrounds_but_not_the_page() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut c = crate::style::computed::BoxStyle::default();
        c.apply_one(
            "background-image",
            "linear-gradient(red, red), linear-gradient(yellow, yellow)",
            vcx,
        );
        c.apply_one("background-clip", "text", vcx);
        c.apply_one("background-color", "red", vcx);
        c.apply_one("background-blend-mode", "difference, normal", vcx);
        collapse(&mut c, vcx, &vcx.background_images());
        assert!(c.background.is_none());
        assert_eq!(c.flat_text_background, Color::parse("lime", vcx));
        assert!(c.bg_layers(vcx).is_none());
    }

    #[test]
    fn a_tiled_constant_gradient_still_only_colors_glyphs() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut c = crate::style::computed::BoxStyle::default();
        c.apply_one("background-image", "linear-gradient(green, green)", vcx);
        c.apply_one("background-color", "red", vcx);
        c.apply_one("background-clip", "text", vcx);
        collapse(&mut c, vcx, &vcx.background_images());
        assert!(c.background.is_none());
        assert_eq!(c.flat_text_background, Color::parse("green", vcx));
    }

    #[test]
    fn unavailable_lower_image_keeps_the_background_color() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut c = crate::style::computed::BoxStyle::default();
        c.apply_one(
            "background-image",
            "linear-gradient(lime, lime), url(data:,invalid-image)",
            vcx,
        );
        c.apply_one("background-color", "cyan", vcx);
        c.apply_one("background-clip", "text, border-box", vcx);
        c.apply_one("background-blend-mode", "difference, lighten", vcx);
        let node = crate::dom::Element::synthetic("div", c.clone(), Default::default(), vec![]);
        crate::document::image_values::prepare(&[crate::dom::Node::Element(node)], vcx);
        collapse(&mut c, vcx, &vcx.background_images());
        assert_eq!(c.background, Color::parse("cyan", vcx));
        assert_eq!(c.flat_text_background, Color::parse("blue", vcx));
    }
}
