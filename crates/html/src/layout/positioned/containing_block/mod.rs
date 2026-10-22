//! Содержащий блок абсолютных коробок и поздняя расстановка: геометрия щупа статической
//! позиции (`Spot`, `SpotCell`), заместитель, ждущий места в строке (`late`), висячее
//! строчное начало (`hang`), слои содержащего блока сборки (`layers`), разделение раскладки и
//! рисования (`source_paint`) и строка абзаца, отдаваемая щупу (`line_box`).

mod spot;
pub use spot::Spot;
mod late;
pub use late::LatePlace;
pub(crate) use late::placed_bounds;
pub use late::spot_place;
pub use late::spot_probe;
mod hang;
pub use hang::InlineStartHang;
mod source_paint;
pub(crate) use source_paint::{DeferredPaint, split as split_source_paint};
mod layers;
mod oof;
pub(crate) use layers::Escaped;
pub(crate) use layers::LayerScope;
pub use layers::Positioning;
pub(crate) use oof::OofKid;

mod line_box;
pub use line_box::{LineBoxSlot, with_line_box};

pub type SpotCell = std::rc::Rc<std::cell::Cell<Spot>>;

/// The CB-ancestor index of the nearest containing block of an absolute box
/// whose parent's computed style is `parent` (see `Computed::cb_depth`).
pub(crate) fn nearest_cb_index(parent: &crate::style::computed::Computed) -> u16 {
    parent.cb_depth + u16::from(crate::text::inline::establishes_cb(parent))
}
