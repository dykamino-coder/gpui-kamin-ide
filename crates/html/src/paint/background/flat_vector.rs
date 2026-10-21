//! Однотонная SVG-картинка фона без растра: `<svg>` с единственным непрозрачным `<rect>`,
//! покрывающим весь viewBox (SVG 2 §8.2), красится одним квадом видимого пересечения. Так
//! огромные вьюпорты не растрируются и не теряют точность координат; Blink
//! `SVGShapePainter::FillShape` тоже красит геометрию прямоугольника напрямую.

use crate::document::state::ValueCx;
use crate::style::values::value::Color;
use gpui::{Bounds, Corners, Pixels, Window};

/// SVG 2 §8.2 maps the viewBox into the image viewport. An opaque rectangle
/// covering that viewport is a uniform image, even when its natural ratio
/// makes the concrete tile too large for accurate bitmap/path coordinates.
/// Blink SVGShapePainter::FillShape paints rectangle geometry directly.
pub(super) fn color(markup: &str, size: (f32, f32), vcx: ValueCx<'_>) -> Option<Color> {
    let markup = super::with_viewport(markup, size);
    let doc = roxmltree::Document::parse(&markup).ok()?;
    let root = doc.root_element();
    if root.tag_name().name() != "svg"
        || root.tag_name().namespace() != Some("http://www.w3.org/2000/svg")
        || root.attributes().any(|a| {
            !matches!(
                a.name(),
                "width" | "height" | "viewBox" | "preserveAspectRatio" | "fill" | "version"
            )
        })
    {
        return None;
    }
    let vb = root
        .attribute("viewBox")
        .map(|v| {
            v.split([' ', ','])
                .filter(|v| !v.is_empty())
                .map(str::parse::<f64>)
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()
        .ok()?;
    let (x, y, w, h) = match vb.as_deref() {
        Some([x, y, w, h]) if *w > 0.0 && *h > 0.0 => (*x, *y, *w, *h),
        Some(_) => return None,
        None => (0.0, 0.0, f64::from(size.0), f64::from(size.1)),
    };
    let aspect = root
        .attribute("preserveAspectRatio")
        .unwrap_or("xMidYMid meet")
        .trim();
    if ![x, y, w, h].iter().all(|v| v.is_finite())
        || (aspect != "none" && f64::from(size.0) / f64::from(size.1) != w / h)
    {
        return None;
    }
    let mut rect = None;
    for child in root.children().filter(roxmltree::Node::is_element) {
        if child.tag_name().namespace() != root.tag_name().namespace() {
            return None;
        }
        match child.tag_name().name() {
            "title" | "desc" | "metadata" => {}
            "rect" if rect.is_none() => rect = Some(child),
            _ => return None,
        }
    }
    let rect = rect?;
    if rect
        .attributes()
        .any(|a| !matches!(a.name(), "x" | "y" | "width" | "height" | "fill"))
        || rect.children().any(|n| n.is_element())
    {
        return None;
    }
    let length = |name, side, default| -> Option<f64> {
        let Some(raw) = rect.attribute(name) else {
            return default;
        };
        let raw = raw.trim();
        if let Some(v) = raw.strip_suffix('%') {
            Some(v.parse::<f64>().ok()? * side / 100.0)
        } else {
            raw.trim_end_matches("px").parse().ok()
        }
    };
    let rx = length("x", w, Some(0.0))?;
    let ry = length("y", h, Some(0.0))?;
    let rw = length("width", w, None)?;
    let rh = length("height", h, None)?;
    if ![rx, ry, rw, rh].iter().all(|v| v.is_finite())
        || rw <= 0.0
        || rh <= 0.0
        || rx > x
        || ry > y
        || rx + rw < x + w
        || ry + rh < y + h
    {
        return None;
    }
    let color = Color::parse(
        rect.attribute("fill")
            .or_else(|| root.attribute("fill"))
            .unwrap_or("black"),
        vcx,
    )?;
    (color.a == 1.0).then_some(color)
}

pub(super) fn paint(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    corners: Corners<Pixels>,
    source: &super::Source,
    vcx: ValueCx<'_>,
) -> bool {
    let super::Source::Vector { markup, .. } = source else {
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
