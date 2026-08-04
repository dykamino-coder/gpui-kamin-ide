//! Иконки: наборы ассетов и подбор иконки по имени файла/папки.
//!
//! `cat` — Catppuccin-набор дерева файлов (таблицы лежат данными рядом с
//! ассетами); `assets` — сгенерированная сборкой таблица «путь → байты SVG».

pub mod cat;

/// Таблица SVG-ассетов Catppuccin, собранная `build.rs` из папки
/// `assets/icons/cat` (см. комментарий там же).
pub mod assets {
    include!(concat!(env!("OUT_DIR"), "/cat_assets.rs"));
}

pub use assets::CAT_ICONS;
pub use cat::{file_icon, folder_icon};
