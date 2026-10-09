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
mod rectangular_clip;
mod legacy_clip;
mod polygon_clip;
mod mask_size;
mod transform_geometry;
mod mask_geometry;
