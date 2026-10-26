//! Таблицы (CSS 2.1 §17, css-tables-3): драйвер раскладки — одна сетка на таблицу: дети и
//! анонимные объекты, `thead`/`tfoot`, ряды и колонки, ширины и кромки, слои фонов, ряды.

use crate::dom::Element;
use crate::layout::options::RenderOpts;
use crate::layout::table::anon::{RowCarry, collect_rows, fixup_table_children};
use crate::layout::table::columns::col_element_widths;
use crate::style::computed::BoxStyle;
use crate::style::values::value::Len;
use gpui::AnyElement;

pub mod anon;
mod axes;
pub mod columns;
pub mod probes;

mod context;
mod finish;
mod table_border_widths;
mod table_clipped_content;
mod table_roles;
mod table_spanning_size;
use crate::layout::table::finish::*;
mod cell_borders;
pub(super) mod rows;
use crate::layout::table::cell_borders::*;
mod build_rows;
use build_rows::table_rows_with_bands;
mod zero_cols;
use zero_cols::zero_width_cols;
mod col_measure;
pub(crate) mod grid_rows;
use col_measure::measure_col_widths;
mod setup;
use setup::{
    border_spacing_of, cell_paints_over, row_span_in_group, rows_left_and_cols, table_rows_fixed,
};

mod builder;
pub(crate) use builder::table_stack;

mod entry;
pub(super) use entry::is_cell;
pub(crate) use entry::table;
