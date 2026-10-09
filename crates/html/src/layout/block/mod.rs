//! Блочный поток: схлопывание полей, струны, переупорядочение, ширина содержащего блока.
// owner: A

pub mod containing;
pub mod margins;
pub mod reorder;
pub mod struts;
mod margin_inline_boxes;
mod margin_edges;
pub(crate) mod margin_height;
pub(crate) mod vertical_flow_margins;
pub(crate) mod available_width;
pub(super) mod ratio_basis;
