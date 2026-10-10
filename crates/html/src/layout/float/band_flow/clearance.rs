//! Resolve clearance when the block-start margin adjoins preceding floats.

use crate::layout::float::bands::FloatBands;

/// CSS 2.1 §9.5.2: test the hypothetical position with `clear: none`.
/// An adjoining start margin would move both the block and the preceding
/// floats. Once clearance separates that margin, place the border edge at
/// the floats' bottom; the clearance may consequently be negative.
/// Blink block_layout_algorithm.cc:1883-1908 likewise distinguishes adjoining
/// and non-adjoining offsets before resolving the BFC position.
pub(super) fn top(
    bands: &FloatBands,
    clear: Option<i8>,
    y: f32,
    margin: f32,
    adjoining: bool,
) -> f32 {
    if clear.is_some() && adjoining && bands.bottom(clear) > y {
        bands.bottom(clear)
    } else {
        bands.clearance(clear, y + margin)
    }
}
