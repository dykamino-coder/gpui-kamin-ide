//! Блочный поток: схлопывание полей, струны, переупорядочение, ширина содержащего блока.

pub(crate) mod available_height;
pub(crate) mod available_width;
pub mod containing;
pub(crate) mod inline_splits;
mod margin_edges;
pub(crate) mod margin_height;
mod margin_inline_boxes;
pub mod margins;
pub(super) mod ratio_basis;
pub mod reorder;
pub(crate) mod sizing;
pub mod struts;
pub(crate) mod vertical_flow_margins;
