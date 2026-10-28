//! Тесты точек разрыва сетки по заданным разложенным дорожкам, border box элементов и рядам их
//! размещения.

use super::grid::RowPush;
use super::{BlockBox, Laid, Pushed};
use crate::dom::Node;
use crate::layout::fragment::flow::FlowGeometry;
use crate::layout::fragment::flow::blocks::MeasuredBreaks;

/// Сетка — первый `div` документа; номера её элементов по порядку.
pub(super) fn grid(html: &str) -> (BlockBox, Vec<u64>) {
    let doc = crate::dom::parse(html, "");
    fn div(n: &Node) -> Option<&crate::dom::Element> {
        let Node::Element(e) = n else { return None };
        if e.tag == "div" {
            return Some(e);
        }
        e.children.iter().find_map(div)
    }
    let root = doc.iter().find_map(div).expect("grid root");
    let b = BlockBox::collect(
        root,
        crate::layout::fragment::ShapeCx::COLUMNS,
        doc.values(),
    );
    let ids = b
        .grid
        .as_ref()
        .expect("grid items")
        .items
        .iter()
        .map(|i| i.id)
        .collect();
    (b, ids)
}

/// Разложенная сетка: border box элементов `(id, верх, низ, первый ряд, последний ряд)`.
fn laid(b: &BlockBox, items: &[(u64, f32, f32, usize, usize)], rows: &[(f32, f32)]) -> Laid {
    let boxes: Vec<_> = items.iter().map(|&(id, t, b, _, _)| (id, t, b)).collect();
    let mut laid = Laid::of_boxes(&boxes);
    laid.grids.insert(b.id, rows.to_vec());
    for &(id, _, _, first, last) in items {
        laid.placements.insert(id, (first, last));
    }
    laid
}

fn breaks(
    b: &BlockBox,
    items: &[(u64, f32, f32, usize, usize)],
    rows: &[(f32, f32)],
    end: f32,
) -> MeasuredBreaks {
    b.breaks(end, &laid(b, items, rows))
        .finish(FlowGeometry::default(), end)
}

#[test]
fn item_break_before_lands_on_its_row_start() {
    // css-grid-1 §12: break-before on an item is propagated to its row.
    let (b, ids) = grid(
        "<div style='display:grid'><div style='height:40px'></div><div style='height:30px;break-before:column'></div></div>",
    );
    let m = breaks(
        &b,
        &[(ids[0], 0.0, 40.0, 0, 0), (ids[1], 50.0, 80.0, 1, 1)],
        &[(0.0, 40.0), (50.0, 80.0)],
        80.0,
    );
    assert_eq!(m.forced, vec![50.0]);
    // The gap between the rows is truncated at a break.
    assert_eq!(m.cuts, vec![(40.0, 50.0)]);
}

#[test]
fn rows_come_from_the_placement_not_from_the_box() {
    // An end-aligned item spanning two rows sits in the second row only physically: its
    // break-after belongs to the end of its grid area (css-grid-2 §8, §12).
    let (b, ids) = grid(
        "<div style='display:grid;grid-template-rows:50px 50px 50px'><div style='grid-row:1/span 2;height:40px;align-self:end;break-after:column'></div><div style='grid-row:3'></div></div>",
    );
    let m = breaks(
        &b,
        &[(ids[0], 60.0, 100.0, 0, 1), (ids[1], 100.0, 150.0, 2, 2)],
        &[(0.0, 50.0), (50.0, 100.0), (100.0, 150.0)],
        150.0,
    );
    assert_eq!(m.forced, vec![100.0]);
}

#[test]
fn monolithic_item_moves_its_whole_row() {
    // An item whose first content cannot break starts its row's range: the
    // break goes before the row (Blink breakpoint row), not just the item.
    let (b, ids) = grid(
        "<div style='display:grid;grid-template-columns:50px 50px'><div style='height:40px'></div><div style='height:40px'></div><div style='height:60px'></div><div style='margin-top:10px;height:50px;contain:size'></div></div>",
    );
    let m = breaks(
        &b,
        &[
            (ids[0], 0.0, 40.0, 0, 0),
            (ids[1], 0.0, 40.0, 0, 0),
            (ids[2], 40.0, 100.0, 1, 1),
            (ids[3], 50.0, 100.0, 1, 1),
        ],
        &[(0.0, 40.0), (40.0, 100.0)],
        100.0,
    );
    assert!(m.solid.contains(&(40.0, 100.0)), "{:?}", m.solid);
}

#[test]
fn avoided_row_boundary_is_reached_only_through_a_moved_row() {
    // break-before: avoid on the third row: a break moved to its start goes
    // to the start of the second row; the edge inside the second row itself
    // is not held by the range.
    let (b, ids) = grid(
        "<div style='display:grid'><div style='height:50px'></div><div style='height:40px'></div><div style='break-before:avoid;height:60px;contain:size'></div></div>",
    );
    let m = breaks(
        &b,
        &[
            (ids[0], 0.0, 50.0, 0, 0),
            (ids[1], 50.0, 90.0, 1, 1),
            (ids[2], 90.0, 150.0, 2, 2),
        ],
        &[(0.0, 50.0), (50.0, 90.0), (90.0, 150.0)],
        150.0,
    );
    assert_eq!(m.avoid, vec![(50.0, 90.05)]);
    assert!(m.solid.contains(&(90.0, 150.0)));
    assert!(!m.solid.iter().any(|s| s.0 < 90.0 && s.1 > 50.0));
}

#[test]
fn row_push_targets_the_track_before_the_row() {
    let (b, ids) = grid(
        "<div style='display:grid'><div style='height:50px'></div><div style='height:40px'></div><div style='grid-row:1/3'></div></div>",
    );
    let rows = [(0.0, 50.0), (50.0, 90.0)];
    let mut items = vec![
        (ids[0], 0.0, 50.0, 0, 0),
        (ids[1], 50.0, 90.0, 1, 1),
        (ids[2], 0.0, 90.0, 0, 1),
    ];
    assert_eq!(
        b.pushed_at(50.0, &laid(&b, &items, &rows)),
        Some(Pushed::Row(RowPush {
            grid: b.id,
            track: 0,
        }))
    );
    assert_eq!(b.pushed_at(20.0, &laid(&b, &items, &rows)), None);
    // Only a spanning item crosses the boundary: the point lies inside it.
    items.remove(0);
    assert_eq!(b.pushed_at(50.0, &laid(&b, &items, &rows)), None);
}

#[test]
fn subgrid_rows_are_pushed_on_the_parent_grid() {
    let (b, ids) = grid(
        "<div style='display:grid'><div style='display:grid;grid-template-rows:subgrid;grid-row:span 2'><div style='height:50px'></div><div style='height:40px'></div></div></div>",
    );
    let sub = b.grid.as_ref().unwrap().items[0].inner.as_ref().unwrap();
    assert!(sub.grid.as_ref().unwrap().inherits_rows);
    let kids: Vec<u64> = sub
        .grid
        .as_ref()
        .unwrap()
        .items
        .iter()
        .map(|i| i.id)
        .collect();
    let rows = [(0.0, 50.0), (50.0, 90.0)];
    let mut laid = laid(&b, &[(ids[0], 0.0, 90.0, 0, 1)], &rows);
    laid.grids.insert(sub.id, rows.to_vec());
    for (kid, (t, bottom, row)) in kids.iter().zip([(0.0, 50.0, 0), (50.0, 90.0, 1)]) {
        laid.boxes.insert(*kid, (t, bottom));
        laid.placements.insert(*kid, (row, row));
    }
    assert_eq!(
        b.pushed_at(50.0, &laid),
        Some(Pushed::Row(RowPush {
            grid: b.id,
            track: 0,
        }))
    );
}

#[test]
fn empty_grid_keeps_native_tracks_and_their_break_boundaries() {
    let (b, ids) = grid("<div style='display:grid;grid-template-rows:auto 1fr 10%'></div>");
    assert!(ids.is_empty());
    assert!(!b.is_empty());
    assert!(b.ids().contains(&b.id));
    let mut laid = Laid::of_boxes(&[(b.id, 0.0, 137.0)]);
    laid.grids
        .insert(b.id, vec![(0.0, 61.0), (61.0, 120.0), (120.0, 137.0)]);
    assert_eq!(b.pushed_at(61.0, &laid), None);
    let m = b
        .breaks(137.0, &laid)
        .finish(FlowGeometry::default(), 137.0);
    assert!(m.cuts.is_empty());
    assert!(m.solid.is_empty());
    assert!(m.forced.is_empty());
    // В пустых дорожках нет содержимого, которое нужно вытолкнуть.
    // Разрез не переразрешает auto/fr/percent и не меняет align-content.
    let kid = crate::layout::fragment::types::Kid {
        h: 137.0,
        over: 137.0,
        cuts: m.cuts,
        solid: m.solid,
        forced: m.forced,
        ..Default::default()
    };
    for height in [50.0, 61.0, 100.0] {
        assert!(
            crate::layout::multicol::column_stack::ColumnStack::growths(
                std::slice::from_ref(&kid),
                2,
                Some(height),
                None,
                8,
            )
            .is_empty()
        );
    }
}
