//! Картинки как ресурс, независимый от фаз: декодирование байт в CPU-растр или рисунок
//! SVG (`decode`), параметры и значения (`types`), природный размер (`natural`), EXIF
//! (`orientation`), вшитый профиль (`colour`), размер и вид SVG (`vector`, `svg_view`);
//! таблица запросов документа (`store`) над процессным кэшем по содержимому (`cache`).
//!
//! Раскладка получает отсюда размеры и альфу, покраска — точки, стиль — факты через
//! `BackgroundImageValues`; модуль не импортирует ни `style`, ни фазы, ни GPUI.

pub mod admission;
pub mod cache;
pub mod colour;
pub mod decode;
pub mod natural;
pub mod orientation;
pub mod store;
pub mod svg_view;
pub mod types;
pub mod vector;

#[cfg(test)]
mod tests;
#[cfg(any(test, feature = "engine-test-support"))]
pub mod tests_support;

pub mod identity;
pub mod pixels;
pub mod svg_fragment;
pub mod svg_links;
pub mod svg_raster;
pub mod viewport;

#[cfg(any(test, feature = "engine-test-support"))]
pub mod fixtures;

pub mod resample;
pub mod sampling;
