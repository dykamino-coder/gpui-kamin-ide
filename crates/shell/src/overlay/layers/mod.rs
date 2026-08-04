//! Слои overlay-окна: каждый добавляет свой оверлей в общий `layer`.
//!
//! Порядок вызовов в `render` прежний (`plan/100-refactor-250.md`).

pub mod menus_context;
pub mod menus_session;
pub mod modal;
pub mod passive;
pub mod pickers;
pub mod search;
