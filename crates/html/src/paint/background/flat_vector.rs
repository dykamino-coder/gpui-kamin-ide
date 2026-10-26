//! Однотонная SVG-картинка фона без растра: `<svg>` с единственным непрозрачным `<rect>`,
//! покрывающим весь viewBox (SVG 2 §8.2), красится одним квадом видимого пересечения. Так
//! огромные вьюпорты не растрируются и не теряют точность координат; Blink
//! `SVGShapePainter::FillShape` тоже красит геометрию прямоугольника напрямую.

use crate::document::state::ValueCx;
use crate::resources::css_images::Source as CpuSource;
#[cfg(test)]
use crate::style::values::value::Color;
use gpui::{Bounds, Corners, Pixels, Window};

pub(super) use crate::resources::css_images::flat_vector::color;

pub(super) fn paint(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    corners: Corners<Pixels>,
    source: &super::Source,
    vcx: ValueCx<'_>,
) -> bool {
    let super::Source::Image(CpuSource::Vector { markup, .. }) = source else {
        return false;
    };
    if corners != Corners::default()
        || window.current_transformation() != gpui::TransformationMatrix::unit()
    {
        return false;
    }
    let Some(color) = color(
        markup,
        (f32::from(bounds.size.width), f32::from(bounds.size.height)),
        vcx,
    ) else {
        return false;
    };
    // Clip before converting a huge viewport into GPU vertex coordinates.
    // Coverage is constant, so the intersection cannot change its phase.
    let visible = bounds.intersect(&window.content_mask().bounds);
    window.paint_quad(gpui::fill(visible, color.to_hsla()));
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn uniform_coverage_respects_viewport_mapping_and_effects() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let full = "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 2147483647 1' preserveAspectRatio='none'><rect width='100%' height='100%' fill='lime'/></svg>";
        assert_eq!(color(full, (768.0, 256.0), vcx), Color::parse("lime", vcx));
        assert!(
            color(
                &full.replace("preserveAspectRatio='none'", ""),
                (768.0, 256.0),
                vcx
            )
            .is_none()
        );
        assert!(
            color(
                &full.replace("fill='lime'", "fill='lime' filter='url(#f)'"),
                (768.0, 256.0),
                vcx
            )
            .is_none()
        );
        assert!(
            color(
                &full.replace("height='100%'", "height='50%'"),
                (768.0, 256.0),
                vcx
            )
            .is_none()
        );
    }
}
