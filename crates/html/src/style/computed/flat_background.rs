//! Constant background colors and clip fills computed from CSS values and image facts.

use crate::document::state::ValueCx;
use crate::style::computed::{BgRepeat, Computed, Tiling};
use crate::style::values::{composite::over, value::Color};

mod stack;
pub(crate) use stack::collapse as collapse_flat_stack;

/// The image backend supplies facts after currentColor and EXIF resolution.
/// Color composition does not depend on decoded image or GUI element types.
pub(crate) trait BackgroundImageValues {
    /// Whether the source can be displayed, as required by CSS Backgrounds 3 §2.3.
    fn image_available(&self, raw: &super::ImageSource, vcx: ValueCx<'_>) -> bool;
    /// A color only when every image pixel is identical and opaque.
    fn uniform_image_color(&self, raw: &super::ImageSource, vcx: ValueCx<'_>) -> Option<Color>;
}

/// Весь фон коробки одним цветом, если он таков: цвет фона, поверх него
/// одноцветный градиент или одноцветный растр, мощённый без зазоров.
/// `None` — фон узорный (или его нет вовсе).
pub(crate) fn flat_fill(
    c: &Computed,
    vcx: ValueCx<'_>,
) -> Option<crate::style::values::value::Color> {
    if c.gradient.is_some() && c.bg_image.is_some() {
        return None;
    }
    let mut fill = c.background.unwrap_or_default();
    if let Some(g) = &c.gradient {
        let one = g.from;
        let repeat = c.bg_repeat.unwrap_or(BgRepeat::Repeat);
        let covers = |t| matches!(t, Tiling::Repeat | Tiling::Round);
        let complete = c.bg_size == crate::style::computed::BgSize::Auto
            && ((covers(repeat.axis(true)) && covers(repeat.axis(false)))
                || (repeat == BgRepeat::NoRepeat
                    && c.bg_pos == Default::default()
                    && c.bg_origin.is_none()));
        let flat = complete
            && (g.hue != 1
                || matches!(
                    g.space,
                    crate::style::computed::GradSpace::Srgb
                        | crate::style::computed::GradSpace::Linear
                        | crate::style::computed::GradSpace::Oklab
                        | crate::style::computed::GradSpace::Lab
                ))
            && !crate::style::values::color_space::out_of_gamut(one)
            && g.to == one
            && g.stops.iter().all(|s| s.0 == one)
            && g.stops_px.iter().all(|s| s.0 == one)
            && g.stops_raw.iter().all(|s| s.0 == one);
        if !flat {
            return None;
        }
        fill = over(one, fill);
    }
    if let Some(src) = &c.bg_image {
        let rep = c.bg_repeat.unwrap_or(BgRepeat::Repeat);
        let covers = |t: Tiling| matches!(t, Tiling::Repeat | Tiling::Round);
        if !covers(rep.axis(true)) || !covers(rep.axis(false)) {
            return None;
        }
        fill = over(c.uniform_image_color(src, vcx)?, fill);
    }
    (fill.a > 0.0).then_some(fill)
}

/// Заливка `background-clip: text`, которую несёт цвет глифов.
///
/// Только НЕПРОЗРАЧНАЯ: тогда «цвет поверх заливки» тоже непрозрачен, и
/// повторное слияние стилей (`inline::inherit` зовётся цепочкой) даёт тот же
/// цвет — смешение не копится.
pub(crate) fn text_clip_fill(
    c: &Computed,
    vcx: ValueCx<'_>,
) -> Option<crate::style::values::value::Color> {
    if let Some(fill) = c.flat_text_background {
        return Some(fill);
    }
    if c.bg_clip != Some(crate::style::computed::BgClip::Text) {
        return None;
    }
    flat_fill(c, vcx).filter(|f| f.a >= 1.0)
}
