//! Правки дерева после разбора: quirks-проценты, цвета правил, run-in, флоаты во флексе,
//! выравнивание, руби.

pub(crate) mod alignment;
pub(crate) mod flow_items;
pub(crate) mod orthogonal_inline;
pub(crate) mod quirks_height;
mod ruby_boxes;
pub(super) use quirks_height::quirks_percent_heights;
pub mod ruby_normalize;
pub(super) use crate::dom::fixup_tree::alignment::align_self_from_dom_parent;
pub(super) use crate::dom::fixup_tree::alignment::grid_table_items_keep_stretch;
pub(super) use crate::dom::fixup_tree::flow_items::flex_items_lose_float;
pub(super) use crate::dom::fixup_tree::flow_items::fold_run_ins;
pub(super) use crate::dom::fixup_tree::orthogonal_inline::orthogonal_inlines_become_flow_roots;
pub(super) use crate::dom::fixup_tree::ruby_boxes::anonymous_ruby;
pub(super) use crate::dom::fixup_tree::ruby_boxes::own_containing_block;
pub(super) use crate::dom::fixup_tree::ruby_boxes::ruby_box_role;
pub(super) use crate::dom::fixup_tree::ruby_boxes::wrap_misparented_ruby;

use crate::budget::admission::{Admission, Error};
use crate::dom::normalization::{Transform, apply};
use crate::dom::*;

/// `filter: url(#id)` дешёвым слоем рисуется только у коробки БЕЗ содержимого
/// (`paint::effects::filter::FilterLayer`): у коробки с детьми слой лёг бы поверх них.
/// Решается здесь, пока дерево целое — при сборке абсолютные дети уже
/// вынесены в свои слои, и родитель выглядит пустым
/// (`filter-region-transformed-composited-child-001`).
pub(super) fn filter_ref_only_empty(
    admission: &Admission,
    nodes: Vec<Node>,
) -> Result<Vec<Node>, Error> {
    apply(&EmptyFilter, nodes, (), admission)
}

pub(super) struct EmptyFilter;

impl Transform for EmptyFilter {
    type Context = ();

    fn enter(&self, el: &mut Element, _: &(), _: &Admission) -> Result<(), Error> {
        if el.style.cascaded().reset_effects.filter_ref.is_some()
            && el.children.iter().any(|c| match c {
                Node::Element(_) => true,
                Node::Text(t) => !crate::dom::whitespace::blank_text(t),
            })
        {
            el.style.cascaded_mut().reset_effects.filter_ref = None;
        }
        Ok(())
    }
}
