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
pub(crate) mod rectangular_clip;
pub(crate) mod legacy_clip;
pub(crate) mod polygon_clip;
pub(crate) mod mask_size;
pub(crate) mod transform_geometry;
pub(crate) mod mask_geometry;
