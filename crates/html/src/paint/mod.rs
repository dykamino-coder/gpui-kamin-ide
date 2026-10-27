//! Этап покраски: по вычисленному стилю и собранным элементам строятся слои фона и фоновых
//! картинок, `border-image`, декораций (рамки, тени, контуры), эффектов (группы, обрезки,
//! маски, трансформы), линеек промежутков и порядок наложения. Раскладкой не владеет: слои —
//! абсолютные дети или обёртки вокруг уже собранного `AnyElement`.

pub mod apply;
pub mod background;
pub mod border_image;
pub(crate) mod box_tree;
pub(crate) mod cache;
pub(crate) mod controls;
pub(crate) mod coordinator;
pub mod decorations;
pub mod effects;
pub mod gap_rules;
pub(crate) mod positioned;
mod raster;
pub(crate) mod replaced;
pub mod stacking;
pub mod table;

#[cfg(test)]
mod contract_tests;
