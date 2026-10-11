//! Flex rows expose parallel item cuts and sequential row boundaries.

use super::geometry::{collect_rows, px, row_container, row_forced};
use crate::dom::Element;
use crate::layout::fragment::breaks::edge_avoid;
use crate::layout::fragment::shape_contents::finish_row_shape;
use crate::layout::fragment::shape_kids::stack_kids;
use crate::layout::fragment::{LINE_CX, Shape, ShapeCx};
use crate::style::values::value::Len;
pub(crate) fn main_size(c: &Element) -> Option<f32> {
    LINE_CX
        .with(|cx| {
            cx.borrow()
                .as_ref()
                .and_then(|c| c.frames.last())
                .and_then(|f| f.w)
        })
        .or_else(|| {
            let Some(Len::Px(w)) = c.style.width else {
                return None;
            };
            let b = c.style.borders();
            let edges =
                px(c.style.padding.left)? + px(c.style.padding.right)? + px(b.left)? + px(b.right)?;
            Some(if c.style.border_box == Some(true) {
                (w - edges).max(0.0)
            } else {
                w
            })
        })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn measure_rows(
    c: &Element,
    depth: u8,
    cx: ShapeCx,
    unclamp: bool,
    top: f32,
    bot: f32,
    mt: f32,
    mb: f32,
) -> Option<Shape> {
    if !row_container(c) {
        return None;
    }
    let main = main_size(c)?;
    let rows = collect_rows(c, main, depth, cx)?;
    let gap = px(c.style.gap.and_then(|g| g.0))?.max(0.0);
    let mut cuts = Vec::new();
    let mut forced = Vec::new();
    let mut solid = Vec::new();
    if top > 0.0 {
        solid.push((0.0, top));
    }
    let mut inner = Vec::new();
    let mut avoid = Vec::new();
    for row in &rows {
        let mut row_cuts = Vec::new();
        let mut row_forces = Vec::new();
        let mut row_solids = Vec::new();
        for item in &row.items {
            let s = &item.shape;
            row_cuts.extend(s.3.iter().map(|&(a, b)| (s.1 + a, s.1 + b)));
            row_forces.extend(s.4.iter().map(|&a| s.1 + a));
            row_solids.extend(s.5.iter().map(|&(a, b)| (s.1 + a, s.1 + b)));
        }
        inner.push((
            row.height,
            0.0,
            0.0,
            row_cuts,
            row_forces,
            row_solids,
            row_forced(row, false),
            row_forced(row, true),
            0.0,
        ));
        avoid.push((
            row.items.iter().any(|i| edge_avoid(&i.element, false)),
            row.items.iter().any(|i| edge_avoid(&i.element, true)),
        ));
    }
    let n = rows.len();
    let (stacked, _) = stack_kids(
        inner,
        false,
        top,
        &mut cuts,
        &mut forced,
        &mut solid,
        true,
        vec![false; n],
        cx,
        vec![false; n],
        false,
        0.0,
        gap,
        avoid,
        vec![false; n],
        vec![(false, false); n],
        vec![None; n],
        0.0,
        None,
    );
    let y = stacked.map_or(top, |s| s.0);
    let natural = y + bot;
    let h = match c.style.height {
        None | Some(Len::Auto) => natural,
        Some(Len::Px(h)) => {
            let h = crate::layout::fragment::fragment_size::border_size(h, &c.style, top + bot);
            if unclamp { h.max(natural) } else { h }
        }
        _ => return None,
    };
    if !unclamp && h > natural + 0.01 {
        return None;
    }
    finish_row_shape(
        c,
        unclamp,
        cx,
        top,
        bot,
        cuts,
        forced,
        solid,
        Some((y, 0.0, 0.0)),
        0.0,
        h,
        mt,
        mb,
    )
}
