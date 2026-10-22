//! Перенос ряда сетки в следующий фрагментаинер распоркой в дереве копии (css-grid-1 §12.1:
//! ряд, начатый в следующем фрагментаинере, сдвигается вместе с последующими, высота сетки
//! `auto` растёт на сдвиг; Blink `ShiftBreakpointIntoNextFragmentainer` сдвигает смещения
//! рядов, не меняя размеров элементов). Сдвиг — поле block-end элементов, кончающихся в
//! предыдущем ряду: дорожка `auto` растёт с их вкладом, растянутые элементы сохраняют размер
//! (поле съедает прирост дорожки); явная дорожка в точках растёт сама.

use crate::dom::{Element, Node};
use crate::layout::fragment::flow::blocks::RowPush;
use crate::layout::fragment::types::StackAxis;
use crate::style::computed::{Track, TrackSize};
use crate::style::values::value::Len;

/// Сдвинуть ряд `row.track + 1` сетки `row.grid` на `grow`. Ложно, когда
/// сдвиг не выражается полями (поле не в точках) или сетки нет.
pub(super) fn push_row(dom: &mut Element, row: &RowPush, grow: f32, axis: StackAxis) -> bool {
    let Some(grid) = find_mut(dom, row.grid) else {
        return false;
    };
    let items = || {
        grid.children.iter().filter_map(|n| match n {
            Node::Element(k) if row.items.contains(&k.node_id) => Some(k),
            _ => None,
        })
    };
    let end = |k: &Element| match axis {
        StackAxis::Horizontal => k.style.margin.bottom,
        StackAxis::VerticalLr => k.style.margin.right,
        StackAxis::VerticalRl => k.style.margin.left,
    };
    if items().any(|k| !matches!(end(k), None | Some(Len::Px(_)))) {
        return false;
    }
    for n in grid.children.iter_mut() {
        if let Node::Element(k) = n
            && row.items.contains(&k.node_id)
        {
            let margin = match axis {
                StackAxis::Horizontal => &mut k.style.margin.bottom,
                StackAxis::VerticalLr => &mut k.style.margin.right,
                StackAxis::VerticalRl => &mut k.style.margin.left,
            };
            let old = crate::style::values::value::px_lossy_zero(*margin);
            *margin = Some(Len::Px(old + grow));
        }
    }
    if let Some(TrackSize::Single(Track::Px(v))) = grid
        .style
        .grid_rows
        .as_mut()
        .and_then(|r| r.get_mut(row.track))
    {
        *v += grow;
    }
    true
}

fn find_mut(c: &mut Element, id: u64) -> Option<&mut Element> {
    if c.node_id == id {
        return Some(c);
    }
    c.children.iter_mut().find_map(|n| match n {
        Node::Element(k) => find_mut(k, id),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_growth_changes_only_the_physical_block_end_and_logical_track() {
        for axis in [
            StackAxis::Horizontal,
            StackAxis::VerticalLr,
            StackAxis::VerticalRl,
        ] {
            let mut item =
                Element::synthetic("div", Default::default(), Default::default(), vec![]);
            item.node_id = 2;
            item.style.margin.bottom = Some(Len::Px(3.0));
            item.style.margin.right = Some(Len::Px(5.0));
            item.style.margin.left = Some(Len::Px(7.0));
            let mut grid =
                Element::synthetic("div", Default::default(), Default::default(), vec![]);
            grid.node_id = 1;
            grid.style.grid_rows = Some(vec![TrackSize::Single(Track::Px(20.0))]);
            grid.children.push(Node::Element(item));
            let row = RowPush {
                grid: 1,
                items: vec![2],
                track: 0,
            };
            assert!(push_row(&mut grid, &row, 11.0, axis));
            let Node::Element(item) = &grid.children[0] else {
                panic!("item")
            };
            let expected = match axis {
                StackAxis::Horizontal => (14.0, 5.0, 7.0),
                StackAxis::VerticalLr => (3.0, 16.0, 7.0),
                StackAxis::VerticalRl => (3.0, 5.0, 18.0),
            };
            assert_eq!(
                (
                    item.style.margin.bottom,
                    item.style.margin.right,
                    item.style.margin.left
                ),
                (
                    Some(Len::Px(expected.0)),
                    Some(Len::Px(expected.1)),
                    Some(Len::Px(expected.2))
                )
            );
            assert_eq!(
                grid.style.grid_rows,
                Some(vec![TrackSize::Single(Track::Px(31.0))])
            );
        }
    }
}
