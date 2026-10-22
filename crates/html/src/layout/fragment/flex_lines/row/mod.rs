//! Остаток оценки рядов для формы и распорок, предназначенный к замене в F1-6.
mod edges;
pub(crate) mod geometry;
mod intrinsic;
mod line_layout;
pub(crate) use edges::row_break_edge;
mod measure;
pub(crate) use measure::{main_size as row_main_size, measure_rows};
#[cfg(test)]
mod tests;
