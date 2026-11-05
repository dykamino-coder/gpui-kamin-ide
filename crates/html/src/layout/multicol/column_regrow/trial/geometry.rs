//! Числовая геометрия проб восстанавливается без перестроения элементов.
use crate::layout::fragment::types::{Kid, StackChild};
/// Восстановить числовой вход, сохранив элементы и рекордеры.
pub(in crate::layout::multicol::column_regrow) fn restore_geometry(
    child: &mut StackChild,
    k: &Kid,
) {
    child.h = k.h;
    child.over = k.over;
    child.mt = k.mt;
    child.mb = k.mb;
    child.monolith = k.monolith;
    child.cuts = k.cuts.clone();
    child.forced = k.forced.clone();
    child.solid = k.solid.clone();
    child.avoid = k.avoid.clone();
    child.keep = k.keep.clone();
    child.line_keep = k.line_keep.clone();
    child.table_rows = k.table_rows.clone();
    child.force_before = k.force_before;
    child.force_after = k.force_after;
    child.avoid_before = k.avoid_before;
    child.avoid_after = k.avoid_after;
    child.par = k.par;
}
