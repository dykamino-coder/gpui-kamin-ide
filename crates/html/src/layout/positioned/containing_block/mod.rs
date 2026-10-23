//! Содержащий блок абсолютных коробок и поздняя расстановка: геометрия щупа статической
//! позиции (`Spot`, `SpotCell`), заместитель, ждущий места в строке (`late`), висячее
//! строчное начало (`hang`), слои содержащего блока сборки (`layers`) и строка абзаца,
//! отдаваемая щупу (`line_box`). Половину рисования на исходном месте держит
//! `paint::positioned`.

use crate::style::computed::BoxStyle;

mod spot;
pub use spot::Spot;
mod late;
pub use late::LatePlace;
pub use late::spot_place;
pub use late::spot_probe;
mod hang;
pub use hang::InlineStartHang;
mod layers;
mod oof;
pub(crate) use layers::LayerScope;
pub use layers::Positioning;

mod line_box;
pub use line_box::{LineBoxSlot, with_line_box};

pub type SpotCell = std::rc::Rc<std::cell::Cell<Spot>>;

/// The CB-ancestor index of the nearest containing block of an absolute box
/// whose parent's computed style is `parent` (see `Computed::cb_depth`).
pub(crate) fn nearest_cb_index(parent: &BoxStyle) -> u16 {
    parent.cb_depth + u16::from(crate::text::inline::establishes_cb(parent))
}
