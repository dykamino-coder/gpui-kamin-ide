//! Доводка стиля элемента: направление, руби, строчный display, авто-пропорция, размеры из атрибутов.

mod presentational_size;
mod ruby_display;
pub(super) use crate::dom::element_style::presentational_size::apply_presentational_size;
pub(super) use crate::dom::element_style::presentational_size::promote_auto_ratio;
pub(super) use crate::dom::element_style::ruby_display::apply_direction;
pub(super) use crate::dom::element_style::ruby_display::finish_inline_display;
pub(super) use crate::dom::element_style::ruby_display::inlinify_in_ruby;
