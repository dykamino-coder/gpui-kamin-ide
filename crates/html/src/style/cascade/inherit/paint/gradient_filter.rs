//! Apply color filters after sampling a single gradient, preserving its geometry.

use crate::document::state::ValueCx;
use crate::style::computed::{Computed, Filter};

pub(super) fn apply(c: &mut Computed, own: &Computed, f: Filter, vcx: ValueCx<'_>) {
    if own.gradient.is_none() || c.bg_layers(vcx).is_some() || f.color_matrix().is_none() {
        return;
    }
    let Some(raw) = c.gradient_raw.as_deref() else {
        return;
    };
    // Filter Effects 1 §5: the color operations consume the rendered image.
    // Blur remains on the subtree's group; the image source handles colors.
    let list = format!(
        "sepia({}) grayscale({}) saturate({}) invert({}) contrast({}) hue-rotate({}deg) brightness({}) opacity({})",
        f.sepia,
        f.grayscale,
        f.saturate,
        f.invert,
        f.contrast,
        f.hue_rotate,
        f.brightness,
        f.opacity,
    );
    c.bg_image = Some(format!("filter({raw}, {list})"));
    c.gradient = None;
    c.gradient_raw = None;
}
