//! Родной контракт content-box коробки: доля и функциональный `fit-content`
//! прибавляют разрешённые края к используемому размеру (CSS 2.1 §10.2, §10.5),
//! поэтому такой коробке нативная раскладка считает размер по content box.

use crate::style::computed::Computed;
use crate::style::values::value::Len;

/// Percentages and functional fit need the native content-box contract: their
/// used size must add the resolved edges (CSS 2.1 sections 10.2 and 10.5).
pub(crate) fn native_content_box(style: &Computed) -> bool {
    style.border_box != Some(true)
        && ((style.width == Some(Len::FitContent)
            && matches!(style.fit_arg[0], Some(Len::Px(_) | Len::Pct(_))))
            || [
                style.width,
                style.height,
                style.min_width,
                style.max_width,
                style.min_height,
                style.max_height,
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
