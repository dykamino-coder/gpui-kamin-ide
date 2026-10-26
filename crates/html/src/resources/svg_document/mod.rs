//! Подготовка SVG-документов: разметка, опорные коробки и снимки определений масок.
//! Растеризацией и элементами GPUI владеют потребители готовой разметки.

pub(crate) mod clip;
pub(crate) use clip::ClipIds;
pub(crate) mod masks;
pub mod serialize;
pub(crate) mod size;
