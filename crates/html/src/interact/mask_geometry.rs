//! CSS mask image tiles share the device grid of painted background tiles.

use gpui::{Bounds, LayoutId, Pixels, Window, point, px};

pub(super) fn positioning_box(
    src: Option<&str>,
    fallback: Bounds<Pixels>,
    id: LayoutId,
    window: &mut Window,
) -> Bounds<Pixels> {
    let Some(src) = src else { return fallback };
    if crate::css::split_args(src).len() != 1
        || src.starts_with("shape:")
        || src.starts_with("bordershape:")
        || src.contains("snap:")
        || src.contains("def:")
        || src.contains('#')
        || window.current_transformation() != gpui::TransformationMatrix::unit()
    {
        return fallback;
    }
    // Percentage positions reference layout geometry (§7.7), so rounding the
    // box origin first would move a subsequent half-device-pixel offset.
    Bounds {
        origin: window.layout_origin_unrounded(id),
        size: window.layout_size_unrounded(id),
    }
}

pub(super) fn tile(
    source: &crate::background::Source,
    bounds: Bounds<Pixels>,
    window: &Window,
) -> Bounds<Pixels> {
    if matches!(source, crate::background::Source::Shape { .. })
        || window.current_transformation() != gpui::TransformationMatrix::unit()
    {
        return bounds;
    }
    // CSS Masking 1 §7.7–7.8 uses background image positioning and sizing.
    // Snap final absolute edges together, after resolving the logical tile
    // (Blink background_image_geometry.cc:756), to match a painted CSS box.
    let scale = window.scale_factor();
    let edge = |v: Pixels| px((f32::from(v) * scale).round() / scale);
    Bounds::from_corners(
        point(edge(bounds.left()), edge(bounds.top())),
        point(edge(bounds.right()), edge(bounds.bottom())),
    )
}
