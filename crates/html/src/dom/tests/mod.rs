//! Тесты разбора DOM: дерево и селекторы (`tree_parsing`), каскад и стили при
//! разборе (`style_parsing`), селекторы в тени (`shadow_selectors`),
//! предки при обходе соседних поддеревьев (`ancestor_stack`), замена элемента
//! картинкой `filter()` (`filtered_content`). Общие помощники `first_element`
//! и `child_colors` — в `tree_parsing`.

mod ancestor_stack;
mod document_mode;
mod filtered_content;
mod property_registry;
mod shadow_selectors;
mod state_identity;
mod style_parsing;
mod tree_parsing;
use crate::dom::tests::tree_parsing::child_colors;
use crate::dom::tests::tree_parsing::first_element;

use super::*;
