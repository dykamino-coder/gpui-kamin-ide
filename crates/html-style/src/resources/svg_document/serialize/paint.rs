//! Export computed SVG paint and geometry to rasterizer attributes.

use super::escape_attr;
use crate::dom::Element;

#[allow(clippy::too_many_arguments)]
pub(super) fn write_paint(e: &Element, out: &mut String) {
    let has = |name: &str| e.attrs.iter().any(|(k, _)| k == name);
    if e.tag != "svg" {
        if let Some(crate::style::values::value::Len::Px(w)) = e.style.used().reset_box_layout.width
            && !has("width")
        {
            out.push_str(&format!(" width=\"{w}\""));
        }
        if let Some(crate::style::values::value::Len::Px(h)) =
            e.style.used().reset_box_layout.height
            && !has("height")
        {
            out.push_str(&format!(" height=\"{h}\""));
        }
    }
    if let Some(fill) = &e.style.used().reset_svg_math.svg_fill
        && !has("fill")
    {
        out.push_str(" fill=\"");
        escape_attr(fill, out);
        out.push('"');
    }
    if let Some(stroke) = &e.style.used().reset_svg_math.svg_stroke
        && !has("stroke")
    {
        out.push_str(" stroke=\"");
        escape_attr(stroke, out);
        out.push('"');
    }
    if let Some(w) = &e.style.used().reset_svg_math.svg_stroke_width
        && !has("stroke-width")
    {
        let k = match (
            e.style.used().reset_svg_math.svg_non_scaling,
            e.style.used().reset_transforms.transform,
        ) {
            (Some(true), Some(t)) if !has("vector-effect") => (t.lin[0][0] * t.lin[1][1]
                - t.lin[0][1] * t.lin[1][0])
                .abs()
                .sqrt(),
            _ => 1.0,
        };
        match w.trim().trim_end_matches("px").parse::<f32>() {
            Ok(v) if k > 1e-6 && (k - 1.0).abs() > 1e-6 => {
                out.push_str(&format!(" stroke-width=\"{}\"", v / k));
            }
            _ => {
                out.push_str(" stroke-width=\"");
                escape_attr(w, out);
                out.push('"');
            }
        }
    }
    if e.tag != "svg" {
        if let Some(crate::style::values::value::Len::Px(x)) = e.style.used().reset_svg_math.svg_x
            && !has("x")
        {
            out.push_str(&format!(" x=\"{x}\""));
        }
        if let Some(crate::style::values::value::Len::Px(y)) = e.style.used().reset_svg_math.svg_y
            && !has("y")
        {
            out.push_str(&format!(" y=\"{y}\""));
        }
    }
}
