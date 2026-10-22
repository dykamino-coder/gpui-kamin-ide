//! Мера многоколоночника до сборки элементов: план рядов и потолка баланса,
//! строчный размер колонки, высота хвостового ряда после спаннера, подготовка и
//! замер детей стопки колонок. Сборку элементов из этих мер ведёт `render`.

mod children;
mod floats;
mod grid_kid;
mod rows;
mod stack_plan;
mod tail;
mod width;

use children::measure_children;
use floats::block_floats;
pub(crate) use rows::{RowPlan, row_plan};
pub(crate) use stack_plan::{StackPlan, stack_plan};
pub(crate) use tail::tail_rest_height;
use width::column_measure_width;
