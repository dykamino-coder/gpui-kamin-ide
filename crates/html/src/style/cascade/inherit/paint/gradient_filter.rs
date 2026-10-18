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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn element_filter_reaches_radial_samples() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        compare_samples("radial-gradient(red, orange)", vcx);
    }

    #[test]
    fn element_filter_reaches_linear_samples() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        compare_samples("linear-gradient(red, orange)", vcx);
    }

    fn compare_samples(raw: &str, vcx: crate::document::state::ValueCx<'_>) {
        let mut own = Computed::default();
        own.apply_one("background-image", raw, vcx);
        own.apply_one("filter", "invert(1)", vcx);
        let rendered = crate::style::cascade::inherit::inherit(&Computed::default(), &own, vcx);
        let image = crate::paint::background::source(rendered.bg_image.as_deref().unwrap(), vcx)
            .unwrap()
            .raster((7.0, 5.0), vcx)
            .unwrap();
        let expected = crate::paint::background::source(&format!("filter({raw}, invert(1))"), vcx)
            .unwrap()
            .raster((7.0, 5.0), vcx)
            .unwrap();
        assert_eq!(image.as_bytes(0), expected.as_bytes(0));
        assert!(rendered.gradient.is_none());
    }
}
