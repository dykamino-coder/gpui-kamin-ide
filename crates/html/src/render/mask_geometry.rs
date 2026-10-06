//! CSS mask painting extents and reference-box offsets from the border box.

use crate::{computed::Computed, value::Len};

pub(super) fn unclipped(c: &Computed) -> bool {
    // CSS Masking section 7.5: no-clip includes paint outside the element box.
    // Blink css_mask_painter.cc:71-87 includes self-painting descendants;
    // GPUI group buffers already cover the viewport, so retain that extent.
    c.mask_image.is_some() && c.mask_clip == Some(255)
}

pub(super) fn offsets(c: &Computed, kind: Option<u8>) -> [f32; 4] {
    let side = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    let b = c.borders();
    match kind {
        Some(2) => [side(b.top), side(b.right), side(b.bottom), side(b.left)],
        // CSS Masking section 7.5: fill-box on a CSS layout box uses content-box;
        // stroke-box and view-box use border-box.
        Some(3) | Some(4) => [
            side(b.top) + side(c.padding.top),
            side(b.right) + side(c.padding.right),
            side(b.bottom) + side(c.padding.bottom),
            side(b.left) + side(c.padding.left),
        ],
        _ => [0.0; 4],
    }
}
