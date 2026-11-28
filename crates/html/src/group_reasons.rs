//! Paint groups the frame's elements made, by the reason the box needed an
//! offscreen buffer (`paint::effects::grouped_wrapper`). A box takes the first
//! reason that holds, in the order of `REASONS`. Process-wide, read by the
//! DevTools `why` journal; one relaxed add per group built.

use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

/// The reasons, in the order of their counters.
pub const REASONS: [&str; 17] = [
    // Corners no quad draws, by `Computed::ellipse_quad_refusal`: a
    // rounded-box mask.
    "rounded_mask_corner_shape",
    "rounded_mask_calc",
    "rounded_mask_border_shape",
    "rounded_mask_border_image",
    "rounded_mask_border_style",
    "rounded_mask_border_colours",
    "rounded_mask_clip",
    "rounded_mask_inset_shadow",
    "rounded_mask",
    "mask_image",
    "border_shape",
    // `clip-path` shapes and polygons.
    "clip_shape",
    // `clip: rect()`, `inset()`, `xywh()` and box keywords.
    "clip_rect",
    "blur",
    "blend",
    "isolation",
    "other",
];

static COUNTS: [AtomicU64; REASONS.len()] = [const { AtomicU64::new(0) }; REASONS.len()];

/// Count one group for `reason` (a name of `REASONS`; others count as the last).
pub(crate) fn note(reason: &str) {
    let at = REASONS
        .iter()
        .position(|r| *r == reason)
        .unwrap_or(REASONS.len() - 1);
    COUNTS[at].fetch_add(1, Relaxed);
}

/// The reason of a group with these effects: its mask (`rrect_spec`
/// geometry is the rounded box, `bordershape:` the border's shape), then its
/// clip, its filter, its blending, else isolation.
pub(crate) fn of(
    blur: f32,
    blend: u8,
    polygon: bool,
    mask: Option<&crate::paint::effects::grouped_element::GroupMask>,
    clipped: bool,
    clip_shape: bool,
    refusal: Option<&'static str>,
) -> &'static str {
    use crate::paint::effects::grouped_element::GroupMask;
    match mask {
        Some(GroupMask::Images(_)) => "mask_image",
        Some(GroupMask::Geometry(s)) if s.starts_with("bordershape:") => "border_shape",
        Some(GroupMask::Geometry(s)) if s.starts_with("shape:") && !clip_shape => rounded(refusal),
        Some(GroupMask::Geometry(_)) => "clip_shape",
        None if polygon => "clip_shape",
        None if clipped => "clip_rect",
        None if blur > 0.0 => "blur",
        None if blend != 0 => "blend",
        None => "isolation",
    }
}

fn rounded(refusal: Option<&'static str>) -> &'static str {
    match refusal {
        Some("corner_shape") => "rounded_mask_corner_shape",
        Some("calc") => "rounded_mask_calc",
        Some("border_shape") => "rounded_mask_border_shape",
        Some("border_image") => "rounded_mask_border_image",
        Some("border_style") => "rounded_mask_border_style",
        Some("border_colours") => "rounded_mask_border_colours",
        Some("clip") => "rounded_mask_clip",
        Some("inset_shadow") => "rounded_mask_inset_shadow",
        _ => "rounded_mask",
    }
}

/// The counters in the order of `REASONS`, set back to zero.
pub fn take() -> [u64; REASONS.len()] {
    std::array::from_fn(|i| COUNTS[i].swap(0, Relaxed))
}

#[cfg(test)]
mod tests {
    #[test]
    fn an_unknown_reason_counts_as_other() {
        super::note("no such reason");
        assert!(super::take()[super::REASONS.len() - 1] >= 1);
    }
}
