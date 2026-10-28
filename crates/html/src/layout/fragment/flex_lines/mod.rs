//! Края разрыва строк flex-ряда и признаки элементов для разрывов класса A.
//! Членство flex-линий фрагментации даёт DetailedLayoutInfo::Flex через Watched.
mod row;
pub(crate) use row::row_break_edge;
mod items;
pub(crate) use items::{class_a_box, inline_display, item_container};
