//! Синхронный вход и классификация ячейки таблицы.

use super::*;

pub(in crate::layout) fn is_cell(e: &Element) -> bool {
    table_roles::is_cell(e)
}

pub(crate) fn table(e: &Element, inherited: &BoxStyle, opts: &RenderOpts) -> AnyElement {
    reblessive::Stack::new()
        .enter(|stk| table_stack(stk, e, inherited, opts))
        .finish()
}
