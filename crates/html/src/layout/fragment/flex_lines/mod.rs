//! Остаток оценки flex для меры формы и старого пути распорок (переход F1-6).
//! Членство flex-линий фрагментации даёт DetailedLayoutInfo::Flex через Watched.
mod column;
pub(super) use column::flex_gap_rules;
mod row;
pub(crate) use row::row_break_edge;
mod items;
pub(crate) use items::{class_a_box, inline_display, item_container};
