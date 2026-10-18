//! Содержащий блок абсолютных коробок (поздняя расстановка).
// owner: A

use gpui::Pixels;
mod spot;
pub use spot::Spot;
mod late;
pub use late::LatePlace;
pub use late::spot_place;
pub use late::spot_probe;
mod hang;
pub use hang::InlineStartHang;
mod source_paint;
pub(crate) use source_paint::split as split_source_paint;
mod layers;
pub use layers::Positioning;

/// Hand-off of a text line's box `(top, height, start x)` from the paragraph
/// that places a static-position probe as an overlay in that line to the
/// probe: the paragraph fills it right before the probe's prepaint, the probe
/// takes it (`OverlayAt::line_probe`, `LayoutCx::line_box`).
pub type LineBoxSlot = std::rc::Rc<std::cell::Cell<Option<(Pixels, Pixels, Pixels)>>>;

pub type SpotCell = std::rc::Rc<std::cell::Cell<Spot>>;

/// The CB-ancestor index of the nearest containing block of an absolute box
/// whose parent's computed style is `parent` (see `Computed::cb_depth`).
pub(crate) fn nearest_cb_index(parent: &crate::style::computed::Computed) -> u16 {
    parent.cb_depth + u16::from(crate::text::inline::establishes_cb(parent))
}
