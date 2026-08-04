//! Дерево файлов по частям (`plan/100-refactor-250.md`):
//!
//! * `model` — записи, декорации, состояние раскрытия и пути;
//! * `rows` — отрисовка строк;
//! * `header` — шапка панели и её кнопки;
//! * `drag` — перетаскивание файла и призрак под курсором.
//!
//! Сборка панели целиком осталась в `ui::file_list`.

pub mod drag;
pub mod header;
pub mod model;
pub mod root_row;
pub mod row;
pub mod row_menu;
pub mod row_parts;
pub mod rows;
