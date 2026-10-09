//! Блочный поток: схлопывание полей, струны, переупорядочение, ширина содержащего блока.
// owner: A

pub mod containing;
pub mod margins;
pub mod reorder;
pub mod struts;
pub(crate) mod margin_inline_boxes;
pub(crate) mod margin_edges;
pub(crate) mod margin_height;
pub(crate) mod vertical_flow_margins;
pub(crate) mod available_width;
pub(crate) mod ratio_basis;
