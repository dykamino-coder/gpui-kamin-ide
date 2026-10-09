//! Эффекты: группы, трансформы, маски, фильтры.
// owner: A

pub(crate) mod containment_paint;
pub mod filter;
pub mod grouped;
pub mod grouped_element;
mod legacy_clip;
pub mod mask;
mod mask_geometry;
mod mask_size;
pub(crate) mod paint_scope;
mod polygon_clip;
mod rectangular_clip;
pub mod transform;
mod transform_geometry;
pub mod transformed_element;
pub mod underlay;
