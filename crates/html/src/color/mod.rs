//! Цветовая математика, общая для стиля и ресурсов-картинок: матрицы пространств
//! CSS Color 4, кривые sRGB, перевод OKLab и отображение в охват sRGB, разбор
//! матричных профилей ICC. Лист без зависимостей от фаз: его читают `style`
//! (`color()`, `lab()`, `@color-profile`) и `resources` (вшитые профили картинок),
//! поэтому он не лежит ни в одной из них.

mod gamut;
mod icc;
mod icc_profile;
mod matrices;

pub(crate) use gamut::{gamut_map, linear_srgb_to_oklab, oklab_to_srgb};
pub(crate) use icc_profile::{IccProfile, clamp_near_gamut};
pub(crate) use matrices::*;
