//! Картинки как ресурс, независимый от фаз: декодирование байт в CPU-растр или рисунок
//! SVG (`decode`), параметры и значения (`types`), природный размер (`natural`), EXIF
//! (`orientation`), вшитый профиль (`colour`), размер и вид SVG (`vector`, `svg_view`).
//!
//! Раскладка получает отсюда размеры и альфу, покраска — точки, стиль — факты через
//! `BackgroundImageValues`; модуль не импортирует ни `style`, ни фазы, ни GPUI.

pub(crate) mod colour;
pub(crate) mod decode;
pub(crate) mod natural;
pub(crate) mod orientation;
pub(crate) mod svg_view;
pub(crate) mod types;
pub(crate) mod vector;

#[cfg(test)]
mod tests;
