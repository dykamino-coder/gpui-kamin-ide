//! Завершающие нормализации дерева после вычисления стилей.

use super::display_inheritance::resolve_display_inherit;
use super::fixup_grid::*;
use super::fixup_tree::*;
use super::outcome::DocumentFailure;
use super::tree_index::TreeIndex;
use super::{Node, float_tail, grid_static_position, subgrid_used};
use crate::budget::admission::Admission;
use crate::values::ValueCx;

pub(super) fn finish(
    tree: &TreeIndex,
    admission: &Admission,
    mut out: Vec<Node>,
    vcx: ValueCx<'_>,
) -> Result<Vec<Node>, DocumentFailure> {
    // ПЕРВЫМ проходом: табличная починка и подъёмы ниже читают `display`.
    resolve_display_inherit(admission, &mut out, (None, None, None, None, None))?;
    if tree.doc.quirks() {
        super::quirks_color::resolve(admission, &mut out, vcx)?;
    }
    // Anonymous inline-table around orphan table boxes inside inline boxes
    // (CSS 2.1 §17.2.1 step 3); block parents are fixed up by `blocks()`.
    crate::dom::table_boxes::inline_anon_tables(admission, &mut out)?;
    // Лунки, которые умеет taffy, — на путь сетки ДО подъёмов и среза
    // подсетки: дальше они идут тем же кодом, что и сетка.
    lanes_as_grid(admission, &mut out)?;
    subgrid_used::adjust(admission, &mut out)?;
    hoist_grid_abspos(admission, &mut out)?;
    grid_static_position::adjust(admission, &mut out)?;
    flex_items_lose_float(admission, &mut out)?;
    align_self_from_dom_parent(admission, &mut out, None)?;
    grid_table_items_keep_stretch(admission, &mut out)?;
    subgrid_takes_parent_tracks(admission, &mut out)?;
    filter_ref_only_empty(admission, &mut out)?;
    fold_run_ins(admission, &mut out, None)?;
    orthogonal_inlines_become_flow_roots(admission, &mut out, (false, false, false), None)?;
    float_tail::mark(admission, &mut out)?;
    if tree.doc.quirks() {
        quirks_percent_heights(admission, &mut out, None)?;
    }
    super::state_identity::finalize(admission, &out, vcx)?;
    if let Some(failure) = tree.doc.revision_budget().failure() {
        return Err(failure);
    }
    Ok(out)
}
