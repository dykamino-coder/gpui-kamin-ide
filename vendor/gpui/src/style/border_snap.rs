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
    exact: Option<Bounds<Pixels>>,
    widths: Edges<Pixels>,
    scale: f32,
    shift: [f32; 2],
) -> (Bounds<Pixels>, Edges<Pixels>) {
    // `shift` is the device translation of the current transformation: an
    // edge snaps where it lands on the device grid.
    let at = |v: Pixels, t: f32| px(((f32::from(v) * scale + t).round() - t) / scale);
    let (x, y) = (|v: Pixels| at(v, shift[0]), |v: Pixels| at(v, shift[1]));
    let outer = Bounds::from_corners(
        point(x(bounds.left()), y(bounds.top())),
        point(x(bounds.right()), y(bounds.bottom())),
    );
    let e = exact.unwrap_or(bounds);
    let inner_left = x(e.left() + widths.left);
    let inner_top = y(e.top() + widths.top);
    let inner_right = x(e.right() - widths.right);
    let inner_bottom = y(e.bottom() - widths.bottom);
    let widths = Edges {
        left: (inner_left - outer.left()).max(Pixels::ZERO),
        top: (inner_top - outer.top()).max(Pixels::ZERO),
        right: (outer.right() - inner_right).max(Pixels::ZERO),
        bottom: (outer.bottom() - inner_bottom).max(Pixels::ZERO),
    };
    (outer, widths)
}

/// The device translation of a transformation whose linear part is the
/// identity within `f32` error (e.g. a text turn and its inverse around a
/// physical atom in vertical writing): edges stay axis-aligned and can snap.
fn translation_only(m: crate::TransformationMatrix) -> Option<[f32; 2]> {
    let [[a, b], [c, d]] = m.rotation_scale;
    let eps = 1e-5;
    ((a - 1.0).abs() < eps && b.abs() < eps && c.abs() < eps && (d - 1.0).abs() < eps)
        .then_some(m.translation)
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
            && translation_only(window.current_transformation()).is_some()
        {
            // Inner edges come from the unrounded border box when the
            // snapped `bounds` belong to the Div being painted: rounding
            // `round(x) + width` instead of `x + width` moved an inner edge
            // by a device pixel off a clip or box edge at the same exact
            // position (`overflow-clip-margin-013`).
            let exact = window
                .css_exact_bounds
                .filter(|(snapped, _)| *snapped == bounds)
                .map(|(_, exact)| exact);
            let shift = translation_only(window.current_transformation()).unwrap_or([0.0; 2]);
            snap(bounds, exact, border_widths, window.scale_factor(), shift)
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
