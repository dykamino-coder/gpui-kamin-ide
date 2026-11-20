//! The fill of a restyled box as the layer holds it: the plain colour its
//! own painter paints, and whether its decorations stay as painted.

use crate::dom::Element;
use crate::style::computed::BoxStyle;
use gpui::Hsla;

/// The plain colour the box's background paints (`Some(None)`: no fill);
/// `None` for a gradient, an image stack, a flattened stack or a colour
/// clipped apart from the box (`background-clip`), which no single fill of
/// the box's own painter (`Style::paint_box`) carries.
pub(in crate::document::revision) fn painted_background(e: &Element) -> Option<Option<Hsla>> {
    let style = BoxStyle::shared(e.style.used());
    let values = style.paint_background();
    let plain = values.gradient.is_none()
        && values.gradient_image.is_none()
        && values.layers.is_none()
        && values.flat.is_none()
        && values.color_clip.is_none();
    plain.then(|| values.color.map(|c| c.to_hsla()))
}

/// Whether the decorations box `a` painted stay as painted for its new style
/// `b`: they read neither style's background group
/// (`decorations::background_free`), and the values they read besides
/// (`color` for `currentcolor`, the colours of the border sides) are the
/// same. Then the only paint that changes is the box's own fill, which the
/// layer takes as a value. Blink repaints the client's every display item
/// (`display_item_client.h:60`); Gecko builds the frame's items into the
/// retained list again (`RetainedDisplayListBuilder::ProcessFrame`).
pub(super) fn kept_decorations(a: &Element, b: &Element) -> bool {
    use crate::paint::decorations::background_free;
    let (old, new) = (a.style.cascaded(), b.style.cascaded());
    old.inh.ui == new.inh.ui
        && old.reset_border.border_colors == new.reset_border.border_colors
        && [a, b]
            .iter()
            .all(|e| background_free(&BoxStyle::shared(e.style.used())))
}
