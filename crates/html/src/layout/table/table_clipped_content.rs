//! Preserve table-cell content alignment inside its overflow clipping wrapper.

use gpui::{AnyElement, Div, div, prelude::*};

pub(crate) fn wrap(cell: &mut Div, contents: Vec<AnyElement>) -> AnyElement {
    // CSS 2.1 §17.5.3 aligns the cell's contents in the row area. Our full-size
    // clipping wrapper occupies that whole area, so aligning it alone leaves
    // the actual contents at the top (especially in spanning cells). Keep the
    // cell's formatting axis and alignment on the wrapper that lays them out.
    let mut clip = div().overflow_hidden().size_full().flex();
    let outer = cell.style();
    let inner = clip.style();
    inner.flex_direction = outer.flex_direction;
    inner.align_items = outer.align_items;
    inner.justify_content = outer.justify_content;
    clip.children(contents).into_any_element()
}
