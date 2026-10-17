//! Алгоритмы раскладки: блок, флоаты, колонки, фрагменты, страницы, таблицы, позиционирование.
// owner: coordinator

pub mod atom;
pub mod block;
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
// Writing-mode algorithms are maintained separately; preserve their explicit
// axis predicates and staged style construction during this cleanup.
#[allow(clippy::nonminimal_bool, clippy::field_reassign_with_default)]
pub mod writing_mode;
