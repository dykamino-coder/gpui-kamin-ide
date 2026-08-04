//! Разбор `ShellEvent` по доменам: `dispatch` выбирает адресата, соседние
//! модули содержат сами обработчики.

pub mod connection;
pub mod customize_pages;
pub mod dispatch;
pub mod editor;
pub mod extensions_updates;
pub mod find_in_files;
pub mod layout_panels;
pub mod layout_presets;
pub mod misc;
pub mod output;
pub mod overlays;
pub mod sessions;
pub mod terminal;
pub mod tree_decorations;
pub mod tree_interaction;
pub mod tree_listing;
pub mod webview;
