//! Синхронный вход в построитель общей коробки с явным стеком.

use super::*;

pub(in crate::render) fn generic_box<const QUERY: bool>(
    e: &Element,
    merged: BoxStyle,
    inherited: &BoxStyle,
    opts: &RenderOpts,
    outer_space: Option<Rc<Nested>>,
) -> AnyElement {
    reblessive::Stack::new()
        .enter(|stk| generic_box_stack::<QUERY>(stk, e, merged, inherited, opts, outer_space))
        .finish()
}
