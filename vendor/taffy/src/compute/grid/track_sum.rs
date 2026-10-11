//! Sum intrinsic grid tracks with one rounding step at the container boundary.
use super::types::GridTrack;

/// CSS Grid §5.2 defines the intrinsic size as the sum of its tracks. Accumulating
/// in f32 can underfill a spanning contribution after it is split between tracks,
/// making equivalent grids disagree at a device-pixel rounding boundary.
pub(super) fn base_size_sum(tracks: &[GridTrack]) -> f32 {
    tracks
        .iter()
        .map(|track| f64::from(track.base_size))
        .sum::<f64>() as f32
}
