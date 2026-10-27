//! Распространение `break-*` с крайних строк многострочного flex-ряда на контейнер
//! (css-flexbox-1 §10) по сбору строк до раскладки; геометрию строк фрагментации даёт
//! final metadata решателя (`paginate::watch`).
mod container;
mod edges;
mod intrinsic;
mod line_layout;
pub(crate) use edges::row_break_edge;
#[cfg(test)]
mod tests;
