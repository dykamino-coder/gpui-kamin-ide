//! Сайдбар сессий, разложенный по частям (`plan/100-refactor-250.md`):
//!
//! * `actions` — что происходит по клику (активация, создание, меню);
//! * `row` — строка сессии;
//! * `header` — шапка проекта и переключатель неактивных;
//! * `pill` / `pill_menu` — hover-пилюля действий и её кнопки.
//!
//! Сборка целиком осталась в `ui::sessions_list`.

pub mod actions;
pub mod glyphs;
pub mod header;
pub mod pill;
pub mod pill_menu;
pub mod rename_row;
pub mod row;
pub mod status_dot;
