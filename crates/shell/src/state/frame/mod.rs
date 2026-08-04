//! Пролог кадра: что RootView делает ДО сборки дерева элементов.
//!
//! Порядок вызовов в render прежний; каждый блок живёт в своём файле
//! (`plan/100-refactor-250.md`).

pub mod editor;
pub mod focus;
pub mod inputs;
pub mod overlay;
pub mod panels;
pub mod webviews;
