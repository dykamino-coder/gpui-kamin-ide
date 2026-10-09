//! CSS mask images are positioned in the unrounded positioning box; tile
//! edges are snapped afterwards by `mask_size::snap_tile`.

use gpui::{Bounds, LayoutId, Pixels, Window};

pub(crate) fn positioning_box(
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
