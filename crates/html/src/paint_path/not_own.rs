//! The exact reason of each refusal counted in `REFUSED_NOT_OWN`: why the
//! restyled box's paint is not its own commands only
//! (`paint::box_tree::styled::own_box_refusal`), or which part of an inline
//! box's band the scene layer does not take
//! (`document::revision::layer_write::own_paint`). One counter per reason.

use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

/// The reasons, in the order of their counters; a reason not listed counts
/// as the last.
pub const REASONS: [&str; 14] = [
    "root_canvas",
    "hover",
    "inline",
    "hidden",
    "clone",
    "filter",
    "transform",
    "decoration",
    // An inline box under a filter, a transform, a clone or `hidden`.
    "band_effect",
    // A change of an inline box's band that is no plain background colour.
    "band_paint",
    // A band appearing or going on a box nested in another inline box.
    "band_nested",
    // A band appearing or going with padding and no border.
    "band_geometry",
    // A box that changed between an own box and an inline band.
    "kind",
    "other",
];

static COUNTS: [AtomicU64; REASONS.len()] = [const { AtomicU64::new(0) }; REASONS.len()];

/// Count one refusal for `reason`.
pub(crate) fn add(reason: &str) {
    let at = REASONS
        .iter()
        .position(|r| *r == reason)
        .unwrap_or(REASONS.len() - 1);
    COUNTS[at].fetch_add(1, Relaxed);
    #[cfg(test)]
    super::local::add(super::PATH + at, 1);
}

/// The counters in the order of `REASONS`.
#[cfg_attr(test, allow(dead_code))]
pub(super) fn read() -> [u64; REASONS.len()] {
    std::array::from_fn(|i| COUNTS[i].load(Relaxed))
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_reason_is_named_after_the_path_counters() {
        for (i, reason) in super::REASONS.iter().enumerate() {
            assert_eq!(
                super::super::NAMES[super::super::PATH + i],
                format!("not_own_{reason}")
            );
        }
    }
}
