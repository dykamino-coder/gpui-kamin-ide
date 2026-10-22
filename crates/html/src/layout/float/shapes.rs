//! Формы вырезов обтекания: построение `FloatShape` из `shape-outside` (`shape`). Сам тип
//! формы и её срез на полосе строки живут в `style::values::float_shape` (их хранит
//! `Computed::flow_shapes`); здесь — переходный реэкспорт для потребителей layout.

mod profile;
mod rounded_float;
mod shape;
pub(crate) use shape::float_shape_of;

pub use crate::style::values::float_shape::FloatShape;
pub(super) use crate::style::values::float_shape::ellipse_cut;
