//! Контуры фигур: border-shape (путь, маска, кольцо, тень, обводка), rrect.

mod border_outline;
mod border_scene;
pub(crate) use border_outline::border_shape_outline_scene;

mod border_shadow;
pub(crate) use border_shadow::border_shape_shadow_scene;

mod border_masks;
pub(crate) use border_masks::border_shape_mask_scene;
pub(crate) use border_masks::border_shape_ring_scene;

mod border_geometry;
pub use border_geometry::motion_shape_d;
pub use border_geometry::rrect_spec;

pub use crate::style::computed::shape_kind::shape_is_linear;
