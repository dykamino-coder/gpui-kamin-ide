//! Snap the outer and inner edges of axis-aligned CSS borders independently.

use crate::{
    BorderStyle, Bounds, ContentMask, Corners, Edges, Pixels, Style, Window, point, px, quad,
};

/// CSS 2.1 §8.5.3 permits device-dependent border rasterization. Blink uses
/// separately snapped outer and inner rectangles (box_border_painter.cc:1357
/// and contoured_border_geometry.cc, PixelSnappedContouredInnerBorder).
/// Snapping the width alone would move the inner edge incorrectly when the
/// border-box origin falls between device pixels.
pub(super) fn snap(
    bounds: Bounds<Pixels>,
    widths: Edges<Pixels>,
    scale: f32,
) -> (Bounds<Pixels>, Edges<Pixels>) {
    let at = |v: Pixels| px((f32::from(v) * scale).round() / scale);
    let outer = Bounds::from_corners(
        point(at(bounds.left()), at(bounds.top())),
        point(at(bounds.right()), at(bounds.bottom())),
    );
    let inner_left = at(bounds.left() + widths.left);
    let inner_top = at(bounds.top() + widths.top);
    let inner_right = at(bounds.right() - widths.right);
    let inner_bottom = at(bounds.bottom() - widths.bottom);
    let widths = Edges {
        left: (inner_left - outer.left()).max(Pixels::ZERO),
        top: (inner_top - outer.top()).max(Pixels::ZERO),
        right: (outer.right() - inner_right).max(Pixels::ZERO),
        bottom: (outer.bottom() - inner_bottom).max(Pixels::ZERO),
    };
    (outer, widths)
}

/// Paint borders before descendants (CSS 2.1 Appendix E), with optional CSS
/// device-pixel snapping. Other GPUI clients keep their existing geometry.
pub(super) fn paint(
    style: &Style,
    bounds: Bounds<Pixels>,
    corner_radii: Corners<Pixels>,
    rem_size: Pixels,
    window: &mut Window,
) {
    if style.is_border_visible() {
        let border_widths = style.border_widths.to_pixels(rem_size);
        let (bounds, border_widths) = if style.css_border_snap
            && corner_radii.max() == Pixels::ZERO
            && style.border_style == BorderStyle::Solid
            && window.current_transformation() == crate::TransformationMatrix::unit()
        {
            snap(bounds, border_widths, window.scale_factor())
        } else {
            (bounds, border_widths)
        };
        let max_border_width = border_widths.max();
        let max_corner_radius = corner_radii.max();

        let top_bounds = Bounds::from_corners(
            bounds.origin,
            bounds.top_right() + point(Pixels::ZERO, max_border_width.max(max_corner_radius)),
        );
        let bottom_bounds = Bounds::from_corners(
            bounds.bottom_left() - point(Pixels::ZERO, max_border_width.max(max_corner_radius)),
            bounds.bottom_right(),
        );
        let left_bounds = Bounds::from_corners(
            top_bounds.bottom_left(),
            bottom_bounds.origin + point(max_border_width, Pixels::ZERO),
        );
        let right_bounds = Bounds::from_corners(
            top_bounds.bottom_right() - point(max_border_width, Pixels::ZERO),
            bottom_bounds.top_right(),
        );

        let mut background = style.border_color.unwrap_or_default();
        background.a = 0.;
        let quad = quad(
            bounds,
            corner_radii,
            background,
            border_widths,
            style.border_color.unwrap_or_default(),
            style.border_style,
        );

        // KaminIDE patch: четыре полосы-маски — оптимизация перерисовки
        // (полосы не пересекаются, итог равен одному проходу). Под
        // преобразованием они лежат в точках окна, а квад — под матрицей:
        // повёрнутое кольцо резалось полосами неповёрнутой коробки
        // (`2d-rotate-001`: рамка 10px под `rotate(30deg)`). Тогда — один
        // проход без масок.
        if window.current_transformation() != crate::TransformationMatrix::unit() {
            window.paint_quad(quad);
        } else {
            window.with_content_mask(Some(ContentMask { bounds: top_bounds }), |window| {
                window.paint_quad(quad.clone());
            });
            window.with_content_mask(
                Some(ContentMask {
                    bounds: right_bounds,
                }),
                |window| {
                    window.paint_quad(quad.clone());
                },
            );
            window.with_content_mask(
                Some(ContentMask {
                    bounds: bottom_bounds,
                }),
                |window| {
                    window.paint_quad(quad.clone());
                },
            );
            window.with_content_mask(
                Some(ContentMask {
                    bounds: left_bounds,
                }),
                |window| {
                    window.paint_quad(quad);
                },
            );
        }
    }
}
