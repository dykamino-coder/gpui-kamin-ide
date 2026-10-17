//! Замещаемые элементы.
// owner: A

pub(crate) mod canvas_grid;
pub mod iframe;
pub mod image;
pub(super) mod inline_replaced_position;
pub mod limits;
mod measured;
pub(crate) mod media;
pub(crate) use measured::{ControlMeasure, natural_control, natural_control_with};
pub(crate) mod replaced_content;
mod replaced_holder_ratio;
pub(crate) mod replaced_used_style;
pub(crate) mod svg_document_image;
pub(crate) mod svg_percentage_size;
