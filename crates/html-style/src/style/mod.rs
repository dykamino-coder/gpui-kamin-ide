//! Фаза стиля: от текста CSS до вычисленного стиля узла и его применения к
//! GPUI. `css` разбирает таблицы и объявления, `select` сопоставляет селекторы
//! с узлами, `cascade` сводит выигравшие объявления и наследование в
//! `computed::Computed`, `values` держит длины и цвета, `generated` — счётчики
//! и кавычки, `zoom` масштабирует длины, `apply` переводит стиль в элемент GPUI.

pub mod animation;
pub mod background_paint;
pub mod box_input;
pub mod canvas_background;
pub mod cascade;
pub mod computed;
pub mod css;
pub mod decoration_paint;
pub mod flow;
pub mod generated;
pub mod inline_decoration;
pub mod select;
pub mod values;
pub mod zoom;

pub mod font_metrics;
mod publication;
pub use publication::ComputedHandle;

pub mod ua;
