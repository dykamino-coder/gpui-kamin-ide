//! Conservative extent of decoration paint outside the border box, in CSS pixels.
use crate::style::computed::BoxStyle;
use crate::style::values::value::Len;

pub(super) fn em_size(c: &BoxStyle) -> f32 {
    match c.inh.font.font_size {
        Some(Len::Px(v)) => v,
        _ => crate::defaults::MEDIUM_FONT_SIZE_PX,
    }
}
pub(super) fn length(l: Option<Len>, none: f32, em: f32) -> Option<f32> {
    match l {
        None => Some(none),
        Some(Len::Px(v)) => Some(v),
        Some(Len::Em(v)) => Some(v * em),
        Some(_) => None,
    }
}

/// How far beyond its box (or its fragments' bleed) the decorations can
/// reach, CSS pixels: its outer shadows, text shadows and outline. `None`
/// for paint that reaches farther than this knows.
pub(crate) fn outset(c: &BoxStyle) -> Option<f32> {
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
        let em = em_size(c);
        let width = length(o.width, 3., em)?.max(0.);
        let offset = length(o.offset, 0., em)?.max(0.);
        let auto = if o.style == Some(2) { width + 2. } else { 0. };
        out = out.max(width + offset + auto);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_the_same_font_relative_lengths_as_the_outline_painter() {
        assert_eq!(length(None, 3., 16.), Some(3.));
        assert_eq!(length(Some(Len::Em(1.5)), 0., 20.), Some(30.));
        assert_eq!(length(Some(Len::Pct(0.5)), 0., 20.), None);
    }
}
