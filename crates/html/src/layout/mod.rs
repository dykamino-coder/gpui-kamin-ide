//! Алгоритмы раскладки: блок, флоаты, колонки, фрагменты, страницы, таблицы, позиционирование.
// owner: coordinator

pub mod atom;
pub mod block;
pub mod cx;
pub mod float;
pub mod fragment;
pub mod grid;
pub mod list;
pub(crate) mod mathml;
pub mod multicol;
pub mod page;
pub mod positioned;
pub mod replaced;
pub mod table;
pub mod writing_mode;
