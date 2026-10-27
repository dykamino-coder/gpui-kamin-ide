//! Свёртка стопки одноцветных слоёв фона в изоляции с сохранением обрезок `text` и
//! `border-area`: для каждой области обрезки стопка сводится к одному цвету, который затем
//! красят глифы или рамка, а слои-картинки из стиля убираются.
//!
//! CSS Compositing 1 §3.4.3 смешивает слой с нижними слоями фона и цветом фона, но не со
//! страницей за элементом. Вычисление постоянной стопки по областям обрезки избавляет от
//! прямоугольной картинки поверх глифов; картинка, которую нельзя показать, считается `none`
//! (CSS Backgrounds 3 §2.3) и даёт прозрачный чёрный. Частичная альфа свёртке не подлежит.

use super::{BackgroundImageValues, ImageAvailability, flat_fill, over};
use crate::style::computed::{BgClip, BlendMode};
use crate::style::values::value::Color;

mod blend;

pub(crate) fn collapse(
    c: &mut crate::style::background_paint::BackgroundValues,
    orient: bool,
    images: &dyn BackgroundImageValues,
) {
    let Some(stack) = evaluate(c, orient, images) else {
        return;
    };
    c.color = (stack[0].a > 0.0).then_some(stack[0]);
    c.flat = Some(stack);
    c.color_clip = None;
    c.top.clip = None;
    c.top.image = None;
    c.gradient = None;
    c.gradient_image = None;
    c.layers = None;
}

fn evaluate(
    c: &crate::style::background_paint::BackgroundValues,
    orient: bool,
    images: &dyn BackgroundImageValues,
) -> Option<[Color; 3]> {
    let layers = c.layers.as_deref().unwrap_or(std::slice::from_ref(&c.top));
    let special = layers
        .iter()
        .any(|l| matches!(l.clip, Some(BgClip::Text | BgClip::BorderArea)));
    let blended = layers
        .iter()
        .any(|l| l.blend.is_some_and(|v| v != BlendMode::Normal));
    if !special && !blended {
        return None;
    }
    let mut result = [Color::default(); 3];
    for (region, pixel) in result.iter_mut().enumerate() {
        if contains(c.color_clip, region)? {
            *pixel = c.color.unwrap_or_default();
        }
    }
    for layer in layers.iter().rev() {
        let availability = layer
            .image
            .as_ref()
            .map(|image| images.image_available(image, orient));
        if availability == Some(ImageAvailability::Pending) {
            return None;
        }
        let unavailable = availability == Some(ImageAvailability::Failed);
        // CSS Backgrounds 3 §2.3: недоступная картинка даёт прозрачный чёрный.
        let operands = c.layer_operands(layer);
        let color = if unavailable || (layer.image.is_none() && operands.gradient.is_none()) {
            Color::default()
        } else {
            flat_fill(&operands, images)?
        };
        for (region, pixel) in result.iter_mut().enumerate() {
            if contains(layer.clip, region)? {
                *pixel = blend::composite(color, *pixel, layer.blend.unwrap_or(BlendMode::Normal))?;
            }
        }
    }
    // Частичная альфа требует отдельной композиции маски глифов или рамки.
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
        // Обрезка по padding/content требует собственной геометрии.
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::computed::BoxStyle;

    #[test]
    fn text_blends_with_lower_backgrounds_but_not_the_page() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut c = BoxStyle::default();
        c.apply_one(
            "background-image",
            "linear-gradient(red, red), linear-gradient(yellow, yellow)",
            vcx,
        );
        c.apply_one("background-clip", "text", vcx);
        c.apply_one("background-color", "red", vcx);
        c.apply_one("background-blend-mode", "difference, normal", vcx);
        let css = c.style_rc().clone();
        let mut result = crate::style::background_paint::BackgroundValues::from_computed(&c);
        collapse(
            &mut result,
            c.image_orient_none != Some(true),
            &vcx.background_images(),
        );
        assert_eq!(c.style_rc(), &css);
        assert!(result.color.is_none());
        assert_eq!(result.flat.map(|s| s[1]), Color::parse("lime", vcx));
        assert!(result.layers.is_none());
        assert!(c.background_layers().is_some());
    }

    #[test]
    fn a_tiled_constant_gradient_still_only_colors_glyphs() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut c = BoxStyle::default();
        c.apply_one("background-image", "linear-gradient(green, green)", vcx);
        c.apply_one("background-color", "red", vcx);
        c.apply_one("background-clip", "text", vcx);
        let css = c.style_rc().clone();
        let mut result = crate::style::background_paint::BackgroundValues::from_computed(&c);
        collapse(
            &mut result,
            c.image_orient_none != Some(true),
            &vcx.background_images(),
        );
        assert_eq!(c.style_rc(), &css);
        assert!(result.color.is_none());
        assert_eq!(result.flat.map(|s| s[1]), Color::parse("green", vcx));
    }

    #[test]
    fn unavailable_lower_image_keeps_the_background_color() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut c = BoxStyle::default();
        c.apply_one(
            "background-image",
            "linear-gradient(lime, lime), url(data:,invalid-image)",
            vcx,
        );
        c.apply_one("background-color", "cyan", vcx);
        c.apply_one("background-clip", "text, border-box", vcx);
        c.apply_one("background-blend-mode", "difference, lighten", vcx);
        let node = crate::dom::Element::synthetic(
            "div",
            c.clone().into_computed(),
            Default::default(),
            vec![],
        );
        crate::document::image_values::prepare(&[crate::dom::Node::Element(node)], vcx);
        let css = c.style_rc().clone();
        let mut result = crate::style::background_paint::BackgroundValues::from_computed(&c);
        collapse(
            &mut result,
            c.image_orient_none != Some(true),
            &vcx.background_images(),
        );
        assert_eq!(c.style_rc(), &css);
        assert_eq!(result.color, Color::parse("cyan", vcx));
        assert_eq!(result.flat.map(|s| s[1]), Color::parse("blue", vcx));
    }
}
