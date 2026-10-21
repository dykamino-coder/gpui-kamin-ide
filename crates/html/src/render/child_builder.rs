//! Реализация в render контракта раскладки по сборке дочерних элементов.

use crate::layout::build_contracts::ChildBuild;
use crate::render::{blocks::blocks, element::element, paragraph::paragraph};

impl Default for ChildBuild {
    fn default() -> Self {
        ChildBuild::new(
            blocks,
            element,
            paragraph,
            crate::paint::box_tree::styled::styled_div_with,
        )
    }
}
