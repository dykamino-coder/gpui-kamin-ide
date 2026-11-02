//! Допуск скролл-контейнера и его вычисленные оси (CSS Overflow 3 §3).
use crate::style::computed::{Computed, Overflow};

pub(super) fn eligible(style: &Computed) -> bool {
    // Eligibility follows computed overflow; the initially visible opposite
    // axis becomes auto even when it has no declaration (Overflow 3 §3).
    let geometry = &style.reset_box_layout;
    [geometry.overflow_x, geometry.overflow_y]
        .into_iter()
        .any(|o| matches!(o, Some(Overflow::Scroll | Overflow::Hidden)))
}

pub(super) fn scroll_axes(style: &Computed) -> Option<(bool, bool)> {
    let geometry = &style.reset_box_layout;
    let snap = geometry.snap.axis != 0;
    let horizontal = geometry.overflow_x == Some(Overflow::Scroll)
        || snap && geometry.overflow_x == Some(Overflow::Hidden);
    let vertical = geometry.overflow_y == Some(Overflow::Scroll)
        || snap && geometry.overflow_y == Some(Overflow::Hidden);
    (horizontal || vertical).then_some((horizontal, vertical))
}
