//! How far the paint of a box reaches beyond its box: the outset an edit off
//! the screen grows its area by (`revision::offscreen`).

use crate::dom::Element;
use crate::style::computed::BoxStyle;
use crate::style::values::value::Len;
use gpui::{Bounds, ScaledPixels};

pub(super) fn grow(b: Bounds<ScaledPixels>, by: f32) -> Bounds<ScaledPixels> {
    Bounds {
        origin: gpui::point(
            ScaledPixels(b.origin.x.0 - by),
            ScaledPixels(b.origin.y.0 - by),
        ),
        size: gpui::size(
            ScaledPixels(b.size.width.0 + 2. * by),
            ScaledPixels(b.size.height.0 + 2. * by),
        ),
    }
}

fn length(l: Option<Len>, none: f32) -> Option<f32> {
    match l {
        None => Some(none),
        Some(Len::Px(v)) => Some(v),
        Some(_) => None,
    }
}

/// How far beyond its box (or its fragments' bleed) the paint of `e` can
/// reach, CSS pixels: its outer shadows, text shadows and outline. `None`
/// for paint that reaches farther than this knows.
pub(super) fn reach(e: &Element) -> Option<f32> {
    let c = BoxStyle::shared(e.style.used());
    let effects = &c.reset_effects;
    let border = &c.reset_border;
    if border.border_shape.is_some()
        || border.border_image.is_some()
        || effects.filter.is_some()
        || effects.filter_ref.is_some()
        || effects.drop_shadow.is_some()
        || effects.backdrop_blur.is_some()
        || effects.backdrop_color.is_some()
    {
        return None;
    }
    let ink = &c.inh.text_ink;
    let shadows = c
        .paint_shadows()
        .iter()
        .chain(ink.text_shadow.iter())
        .chain(ink.text_shadow_rest.iter());
    let mut out = 0f32;
    for s in shadows {
        out = out.max(s.x.abs().max(s.y.abs()) + 2. * s.blur.max(0.) + s.spread.max(0.));
    }
    if let Some(o) = &border.outline
        && o.style.is_some_and(|s| s != 0)
    {
        // `medium` is 3px; `auto` paints a focus ring beside the width.
        let width = length(o.width, 3.)?.max(0.);
        let offset = length(o.offset, 0.)?.max(0.);
        let auto = if o.style == Some(2) { width + 2. } else { 0. };
        out = out.max(width + offset + auto);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn growing_keeps_the_centre() {
        let b = Bounds {
            origin: gpui::point(ScaledPixels(10.), ScaledPixels(10.)),
            size: gpui::size(ScaledPixels(4.), ScaledPixels(4.)),
        };
        let g = grow(b, 3.);
        assert_eq!(g.origin, gpui::point(ScaledPixels(7.), ScaledPixels(7.)));
        assert_eq!(g.size, gpui::size(ScaledPixels(10.), ScaledPixels(10.)));
    }

    #[test]
    fn a_length_other_than_pixels_is_unknown() {
        assert_eq!(length(None, 3.), Some(3.));
        assert_eq!(length(Some(Len::Px(2.)), 3.), Some(2.));
        assert_eq!(length(Some(Len::Pct(0.5)), 3.), None);
    }
}
