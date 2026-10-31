//! Подготовка SVG-документов: разметка, опорные коробки и снимки определений масок.
//! Растеризацией и элементами GPUI владеют потребители готовой разметки.

pub mod clip;
pub mod foreign_rasters;
pub use clip::ClipIds;
pub mod masks;
pub mod serialize;
pub mod size;
