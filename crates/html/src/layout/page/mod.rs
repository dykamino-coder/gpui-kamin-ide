//! Постраничная раскладка (css-page-3): стопка листов `PageStack`, разбиение содержимого
//! по страницам, именованные страницы, страничные счётчики и марджин-боксы листа
//! с их порождённым содержимым, мерой и раскладкой по краям бумаги.

mod margin_box_size;
mod margin_boxes;
mod margin_layout;
pub mod names;
mod page_boxes;
mod page_counters;
pub mod page_stack;
pub mod paged;
