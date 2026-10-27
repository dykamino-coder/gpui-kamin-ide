//! Родной контракт content-box коробки: доля и функциональный `fit-content`
//! прибавляют разрешённые края к используемому размеру (CSS 2.1 §10.2, §10.5),
//! поэтому такой коробке нативная раскладка считает размер по content box.

use crate::style::computed::Computed;
use crate::style::values::value::Len;

/// Percentages and functional fit need the native content-box contract: their
/// used size must add the resolved edges (CSS 2.1 sections 10.2 and 10.5).
pub(crate) fn native_content_box(style: &Computed) -> bool {
    native_content_box_with(
        style,
        &super::block::sizing::BoxDimensions::from_computed(style),
    )
}

pub(crate) fn native_content_box_with(
    style: &Computed,
    sizes: &super::block::sizing::BoxDimensions,
) -> bool {
    !sizes.border_box
        && ((sizes.preferred[0] == Some(Len::FitContent)
            && matches!(style.fit_arg[0], Some(Len::Px(_) | Len::Pct(_))))
            || [
                sizes.preferred[0],
                sizes.preferred[1],
                sizes.minimum[0],
                sizes.maximum[0],
                sizes.minimum[1],
                sizes.maximum[1],
            ]
            .into_iter()
            .zip(style.stretch_size)
            .any(|(size, stretch)| !stretch && matches!(size, Some(Len::Pct(_))))
            || [
                style.padding.left,
                style.padding.right,
                style.padding.top,
                style.padding.bottom,
            ]
            .into_iter()
            .any(|side| matches!(side, Some(Len::Pct(_)))))
}
