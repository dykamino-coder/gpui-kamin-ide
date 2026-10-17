//! Shared hypothetical row geometry for flex fragmentation (CSS Flexbox sections 9.3/9.4).

use super::line_layout::collect_lines;
use crate::dom::Element;
use crate::layout::fragment::breaks::edge_break;
use crate::layout::fragment::clone::{monolith_box, solid_box};
use crate::layout::fragment::fragment_size::shape_full;
use crate::layout::fragment::{Shape, ShapeCx};
use crate::style::computed::{Align, Display, FlexDir, Justify};
use crate::style::values::value::Len;

pub(crate) struct RowItem {
    pub(crate) source: usize,
    pub(crate) element: Element,
    pub(crate) shape: Shape,
}

pub(crate) struct FlexRow {
    pub(crate) items: Vec<RowItem>,
    pub(crate) height: f32,
}

pub(crate) fn px(value: Option<Len>) -> Option<f32> {
    match value {
        None | Some(Len::Auto) => Some(0.0),
        Some(Len::Px(v)) => Some(v),
        _ => None,
    }
}

pub(crate) fn row_container(c: &Element) -> bool {
    let s = &c.style;
    s.display == Some(Display::Flex)
        && s.webkit_box != Some(true)
        && matches!(s.flex_dir, None | Some(FlexDir::Row))
        && s.flex_wrap == Some(true)
        && s.flex_wrap_reverse != Some(true)
        // Fixed cross-size fragmentation needs row expansion beyond the box's
        // original height. Leave that to its existing placement path.
        && matches!(s.height, None | Some(Len::Auto))
        && s.flex_balance != Some(true)
        && s.vertical != Some(true)
        && s.rtl != Some(true)
        && matches!(
            s.align_items,
            None | Some(Align::Stretch) | Some(Align::Start)
        )
        && matches!(
            s.align_content,
            None | Some(Justify::Start) | Some(Justify::Stretch)
        )
}

pub(crate) fn collect_rows(c: &Element, main: f32, depth: u8, cx: ShapeCx) -> Option<Vec<FlexRow>> {
    let mut rows = Vec::new();
    for line in collect_lines(c, main, depth)? {
        let mut row = FlexRow {
            items: Vec::new(),
            height: 0.0,
        };
        for (source, element) in line {
            let shape = item_shape(&element, depth, cx)?;
            row.height = row
                .height
                .max(shape.height + shape.margin_top + shape.margin_bottom);
            row.items.push(RowItem {
                source,
                element,
                shape,
            });
        }
        rows.push(row);
    }
    for row in &mut rows {
        for item in &mut row.items {
            let s = &mut item.element.style;
            if matches!(s.height, None | Some(Len::Auto))
                && !matches!(s.align_self.or(c.style.align_items), Some(Align::Start))
                && item.shape.height + item.shape.margin_top + item.shape.margin_bottom
                    < row.height - 0.01
            {
                let b = s.borders();
                let edges = px(s.padding.top)? + px(s.padding.bottom)? + px(b.top)? + px(b.bottom)?;
                let target = row.height - item.shape.margin_top - item.shape.margin_bottom;
                let minimum = if s.border_box == Some(true) {
                    target
                } else {
                    (target - edges).max(0.0)
                };
                s.min_height = Some(Len::Px(minimum.max(px(s.min_height)?)));
                item.shape = item_shape(&item.element, depth, cx)?;
            }
        }
    }
    Some(rows)
}

/// A monolith (css-break-4 §4.1) is solid over its whole block size. A box
/// solid only because of its inline content keeps the ranges its line
/// measure reports (`line_run_shape`: lines and orphans/widows groups), so
/// the unused part of a specified height stays fragmentable, as Blink
/// fragments a block's remaining block-size after its last line; unmeasured
/// lines (no ranges) remain one solid range.
fn item_shape(element: &Element, depth: u8, cx: ShapeCx) -> Option<Shape> {
    let mut shape = shape_full(element, depth - 1, cx)?;
    if solid_box(element)
        && shape.cuts.is_empty()
        && (monolith_box(element) || shape.solid.is_empty())
    {
        shape.solid = vec![(0.0, shape.height)];
    }
    Some(shape)
}

pub(crate) fn row_forced(row: &FlexRow, last: bool) -> bool {
    row.items.iter().any(|i| edge_break(&i.element, last))
}
