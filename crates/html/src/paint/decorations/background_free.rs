//! Whether the decorations of a box (`decorations`) paint nothing from its
//! background group, so a change of its background colour alone leaves every
//! decoration element as it was painted.
//!
//! The decorations that read the background: a background image or gradient
//! and their tiles and stripes (`background::layer`, `gradient_stripes`), the
//! colour painted as a layer of its own (`background::color_layer`: a clip
//! apart from the border box, a backdrop), the fill of a single rounded corner
//! (`single_corner`), the border over an image layer (`box_borders`), a border
//! shape's fill and the outer shadows that skip an opaque fill
//! (`shaped_outset`). Coloured border sides and outlines read only their own
//! colours. Blink paints a box's background, borders and outline as display
//! items of one client and invalidates the client whole
//! (`ObjectPaintInvalidator`); here the decorations are elements of their own,
//! so only the box's own fill is painted again and they stay.

use crate::style::computed::BoxStyle;

/// The decorations of `c` read no value of its background group.
pub(crate) fn background_free(c: &BoxStyle) -> bool {
    let bg = c.paint_background();
    let effects = &c.reset_effects;
    bg.gradient.is_none()
        && bg.gradient_image.is_none()
        && bg.layers.is_none()
        && bg.flat.is_none()
        && bg.color_clip.is_none()
        && bg.top.image.is_none()
        && c.paint_shadows().is_empty()
        && c.reset_border.border_shape.is_none()
        && c.reset_border.border_image.is_none()
        && !effects.backdrop_filter_set
        && effects.backdrop_color.is_none()
        && effects.backdrop_ref.is_none()
        && effects.filter_ref.is_none()
        && !crate::paint::background::single_corner::eligible(c)
}
