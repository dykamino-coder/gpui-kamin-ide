//! Интерактивные элементы: прокрутка, изменение размера, липкие коробки.
// owner: coordinator

pub(crate) mod forms;
pub mod frame;
pub mod resizable;
pub mod scroll;
pub mod scroll_area;
pub mod scroll_box;
pub(super) mod scroll_target;
pub(crate) mod scrollbars;
pub mod select;
pub mod sticky;

pub(crate) mod scroll_extent;
pub(crate) mod scrollbar_gutter;

pub(crate) mod selection;

mod drag;
pub(crate) mod hitbox;
