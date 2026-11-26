//! The SVG and MathML group under paint-only damage: `fill` and `stroke` are
//! paint values (Blink `css_properties.json5`: `fill` and `stroke` invalidate
//! `paint`; Gecko `nsStyleSVG::CalcDifference`: `nsChangeHint_RepaintFrame`).
//! Layout here never reads them: the shapes rasterise inside the CSS box of
//! their `svg` (`resources::svg_document::serialize`), so the reflow Gecko
//! adds when a geometry frame gains or loses its fill or stroke has no
//! counterpart. `stroke-width` is a paint value as well: it moves the ink of
//! a shape inside the raster of its `svg`, whose canvas the frame build sizes
//! (`svg::element`), and no CSS box lays out by it (Blink invalidates paint
//! of the shape for a new stroke; Gecko reflows only the SVG geometry frame,
//! which here is that raster). The geometry properties and the MathML values
//! stay layout damage.

use super::super::ResetSvgMath;

/// Whether layout reads the same values from both groups.
pub(super) fn same_layout_svg(a: &ResetSvgMath, b: &ResetSvgMath) -> bool {
    let ResetSvgMath {
        svg_no_css_box,
        svg_fill: _,
        svg_stroke: _,
        svg_stroke_width: _,
        svg_x,
        svg_y,
        svg_non_scaling,
        mathml,
        math_layout,
        math_depth,
        math_font_size,
        svg_geometry,
        svg_path_length,
    } = a;
    *svg_no_css_box == b.svg_no_css_box
        && *svg_x == b.svg_x
        && *svg_y == b.svg_y
        && *svg_non_scaling == b.svg_non_scaling
        && *mathml == b.mathml
        && *math_layout == b.math_layout
        && *math_depth == b.math_depth
        && *math_font_size == b.math_font_size
        && *svg_geometry == b.svg_geometry
        && *svg_path_length == b.svg_path_length
}

#[cfg(test)]
mod tests {
    use super::super::super::Computed;
    use crate::style::values::value::Len;

    #[test]
    fn fill_and_stroke_are_paint_only() {
        let a = Computed::default();
        let mut b = a.clone();
        b.reset_svg_math.svg_fill = Some("#e00".into());
        assert!(a != b && a.same_layout(&b), "fill is paint");
        let mut c = a.clone();
        c.reset_svg_math.svg_stroke = Some("blue".into());
        assert!(a != c && a.same_layout(&c), "stroke is paint");
    }

    #[test]
    fn stroke_width_is_paint_and_geometry_is_layout() {
        let a = Computed::default();
        let mut b = a.clone();
        b.reset_svg_math.svg_stroke_width = Some("3".into());
        assert!(a != b && a.same_layout(&b), "stroke-width is paint");
        let mut c = a.clone();
        c.reset_svg_math.svg_x = Some(Len::Px(1.0));
        assert!(!a.same_layout(&c), "x is geometry");
        let mut d = a.clone();
        d.reset_svg_math.svg_geometry = vec![("r", "4".into())];
        assert!(!a.same_layout(&d), "r is geometry");
    }
}
