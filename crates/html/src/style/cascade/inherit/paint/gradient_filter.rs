//! Apply color filters after sampling a single gradient, preserving its geometry.

use crate::style::computed::{Computed, Filter};

pub(super) fn apply(c: &mut Computed, own: &Computed, f: Filter) {
    if own.gradient.is_none() || c.bg_layers().is_some() || f.color_matrix().is_none() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn element_filter_reaches_radial_samples() {
        compare_samples("radial-gradient(red, orange)");
    }

    #[test]
    fn element_filter_reaches_linear_samples() {
        compare_samples("linear-gradient(red, orange)");
    }

    fn compare_samples(raw: &str) {
        let mut own = Computed::default();
        own.apply_one("background-image", raw);
        own.apply_one("filter", "invert(1)");
        let rendered = crate::style::cascade::inherit::inherit(&Computed::default(), &own);
        let image = crate::paint::background::source(rendered.bg_image.as_deref().unwrap())
            .unwrap()
            .raster((7.0, 5.0))
            .unwrap();
        let expected = crate::paint::background::source(&format!("filter({raw}, invert(1))"))
            .unwrap()
            .raster((7.0, 5.0))
            .unwrap();
        assert_eq!(image.as_bytes(0), expected.as_bytes(0));
        assert!(rendered.gradient.is_none());
    }
}
