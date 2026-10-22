//! Остаток оценки flex для меры формы и старого пути распорок (переход F1-6).
//! Членство flex-линий фрагментации даёт DetailedLayoutInfo::Flex через Watched.
mod column;
pub(super) use column::flex_gap_rules;
mod row;
pub(crate) use row::{geometry as row_geometry, measure_rows, row_break_edge, row_main_size};
mod items;
pub(crate) use items::{class_a_box, inline_display, item_container};
