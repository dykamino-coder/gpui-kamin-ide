//! Замещаемые элементы (CSS 2.1 §10.3.2, css-images-3): картинки, `canvas`, `iframe`,
//! медиа и SVG-документы — их естественные размеры, пропорции, ограничения `min/max`,
//! измеряемый GPUI-элемент, положение в строке и содержимое замещаемой коробки.

pub(crate) mod canvas_grid;
pub mod iframe;
pub mod image;
pub(super) mod inline_replaced_position;
pub mod limits;
pub(crate) mod measured;
pub(crate) mod media;
pub(crate) mod natural;
pub(crate) use measured::{ControlMeasure, natural_control, natural_control_with};
pub(crate) mod replaced_content;
pub(crate) mod replaced_holder_ratio;
pub(crate) mod replaced_used_style;
pub(crate) mod svg_document_image;
pub(crate) mod svg_percentage_size;
