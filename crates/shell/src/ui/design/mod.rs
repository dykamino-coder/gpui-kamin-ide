//! Панель Design по частям (`plan/100-refactor-250.md`):
//!
//! * `layout` — каркас: секции и блоки;
//! * `tokens` — токены (имя, значение, образец цвета);
//! * `samples_*` — живые семплы контролов.
//!
//! Сборка панели целиком осталась в `ui::design_panel`.

pub mod components;
pub mod layout;
pub mod samples_chrome;
pub mod samples_input;
pub mod samples_nav;
pub mod samples_tree;
pub mod tokens;
pub mod tokens_page;
