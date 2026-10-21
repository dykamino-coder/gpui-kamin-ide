//! Коробка позиционирования слоя `mask-image`: для одиночной картинки без трансформа берётся
//! неокруглённая коробка раскладки, так как доли `mask-position` отсчитываются от геометрии
//! раскладки (css-masking-1 §7.7) и округление начала сдвинуло бы полупиксельное смещение;
//! края плитки привязываются потом в `mask_size::snap_tile`.

use gpui::{Bounds, LayoutId, Pixels, Window};

pub(super) fn positioning_box(
    src: Option<&str>,
    fallback: Bounds<Pixels>,
    id: LayoutId,
    window: &mut Window,
) -> Bounds<Pixels> {
    let Some(src) = src else { return fallback };
    if crate::style::css::split_args(src).len() != 1
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
