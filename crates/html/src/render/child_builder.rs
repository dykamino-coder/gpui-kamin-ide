//! Реализация в render контракта раскладки по сборке дочерних элементов.

use crate::layout::build_contracts::{ChildBuild, FlowBuild};
use crate::render::{blocks::blocks, element::element, paragraph::paragraph};

impl Default for ChildBuild {
    fn default() -> Self {
        ChildBuild::new(
            FlowBuild::new(
                blocks,
                paragraph,
                crate::render::generic_box::child_probes,
                crate::render::first_line_descendants::route,
            ),
            element,
            crate::paint::box_tree::styled::styled_div_with,
        )
    }
}
