//! Слои покраски таблицы по геометрии, снятой пробами раскладки (`layout::table::probes`):
//! полосы рядов, групп и колонок, обрезанные ячейками (`CellsClipped`), фоны ячеек сросшейся
//! модели (`CellBgPainter`) и сросшиеся кромки (`EdgePainter`, CSS 2.1 §17.6.2.1).

mod cell_bg;
mod clipped;
mod clipped_paint;
mod edge_painter;
mod edge_segments;
pub use cell_bg::CellBgPainter;
pub use clipped::CellsClipped;
pub use edge_painter::EdgePainter;
