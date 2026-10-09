//! Эффекты: группы, трансформы, маски, фильтры.
// owner: A

pub mod filter;
pub mod grouped;
pub mod grouped_element;
pub mod mask;
pub mod transform;
pub mod transformed_element;
pub mod underlay;
pub(crate) mod containment_paint;
pub(crate) mod paint_scope;
