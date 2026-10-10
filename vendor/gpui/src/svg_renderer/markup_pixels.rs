//! Convert SVG markup texels with exact, nearest-byte unpremultiplication.

/// Source-over paint with one solid color retains that straight color at
/// every coverage (Compositing 1 §5.1). Recover it from the parsed paints,
/// since an eight-bit premultiplied raster has already lost that information.
pub(super) fn solid_color(tree: &usvg::Tree) -> Option<usvg::Color> {
    let mut color = None;
    collect_color(tree.root(), &mut color)
        .then_some(color)
        .flatten()
}

fn collect_color(group: &usvg::Group, color: &mut Option<usvg::Color>) -> bool {
    if group.blend_mode() != usvg::BlendMode::Normal || !group.filters().is_empty() {
        return false;
    }
    for node in group.children() {
        match node {
            usvg::Node::Group(group) => {
                if !collect_color(group, color) {
                    return false;
                }
            }
            usvg::Node::Path(path) if path.is_visible() => {
                let paints = path
                    .fill()
                    .map(|fill| fill.paint())
                    .into_iter()
                    .chain(path.stroke().map(|stroke| stroke.paint()));
                for paint in paints {
                    let usvg::Paint::Color(found) = paint else {
                        return false;
                    };
                    if color.is_some_and(|previous| previous != *found) {
                        return false;
                    }
                    *color = Some(*found);
                }
            }
            usvg::Node::Path(_) => {}
            usvg::Node::Image(_) | usvg::Node::Text(_) => return false,
        }
    }
    true
}

pub(super) fn swap_and_unpremultiply(pixel: &mut [u8]) {
    pixel.swap(0, 2);
    let alpha = u16::from(pixel[3]);
    if alpha == 0 {
        return;
    }
    // SVG raster bytes are premultiplied. The inverse must preserve a
    // saturated channel at every nonzero alpha: a/a is exactly one. Float
    // division followed by truncation turned white into 254 for many alpha
    // values, and CSS Masking 1 §7.10.1 then attenuated white luminance masks.
    // Round other channels to the nearest byte to minimize re-encoding error.
    for channel in &mut pixel[..3] {
        *channel = ((u16::from(*channel) * 255 + alpha / 2) / alpha).min(255) as u8;
    }
}
