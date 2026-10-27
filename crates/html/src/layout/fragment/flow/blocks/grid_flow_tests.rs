//! Тесты элементов сетки как параллельных потоков: что из потока элемента попадает в точки
//! сетки, рост строк элемента у края фрагментаинера и перенос ряда через пустой ряд.

use super::grid::RowPush;
use super::grid_tests::grid;
use super::{BlockBox, Laid, Pushed, grid_tree};
use crate::dom::{Element, Node};
use crate::layout::fragment::flow::{FlowGeometry, LinePush};
use crate::layout::fragment::push::flex_flow::growths;
use std::cell::RefCell;
use std::rc::Rc;

fn first_div(nodes: &[Node]) -> Option<&Element> {
    nodes.iter().find_map(|n| match n {
        Node::Element(e) if e.tag == "div" => Some(e),
        Node::Element(e) => first_div(&e.children),
        _ => None,
    })
}

/// Две строки элемента `item` (`(верх, низ)`) — его собственный поток с адресами роста.
fn item_lines(laid: &mut Laid, item: u64, lines: [(f32, f32); 2]) -> Rc<RefCell<Vec<f32>>> {
    let growth = Rc::new(RefCell::new(Vec::new()));
    let flow = FlowGeometry {
        solid: lines.to_vec(),
        cuts: vec![(lines[0].1, lines[1].0)],
        ..Default::default()
    };
    laid.flows.insert(item, flow);
    laid.lines.insert(
        item,
        lines
            .iter()
            .enumerate()
            .map(|(line, l)| LinePush {
                at: l.0,
                line,
                growth: growth.clone(),
            })
            .collect(),
    );
    growth
}

/// Ряд из двух элементов `0..row`: первый пуст, второй держит строки.
fn row_of_two(row: f32) -> (BlockBox, Vec<u64>, Laid) {
    let (b, ids) = grid(
        "<div style='display:grid;grid-template-columns:25px 25px'><div></div><div>a b</div></div>",
    );
    let mut laid = Laid::of_boxes(&[(ids[0], 0.0, row), (ids[1], 0.0, row), (b.id, 0.0, row)]);
    laid.grids.insert(b.id, vec![(0.0, row)]);
    laid.placements.insert(ids[0], (0, 0));
    laid.placements.insert(ids[1], (0, 0));
    (b, ids, laid)
}

#[test]
fn only_the_first_line_of_an_item_holds_its_row() {
    // css-grid-2 §12: a break inside an item does not break its siblings; only an
    // unbreakable first piece moves the row.
    let (b, ids, mut laid) = row_of_two(180.0);
    item_lines(&mut laid, ids[1], [(0.0, 80.0), (80.0, 180.0)]);
    let m = b
        .breaks(180.0, &laid)
        .finish(FlowGeometry::default(), 180.0);
    assert!(m.solid.contains(&(0.0, 80.0)), "{:?}", m.solid);
    assert!(!m.solid.iter().any(|s| s.0 >= 80.0), "{:?}", m.solid);
    assert!(m.cuts.iter().any(|c| c.0 == 80.0), "{:?}", m.cuts);
}

#[test]
fn a_line_crossing_the_edge_grows_its_own_item_only() {
    let (b, ids, mut laid) = row_of_two(180.0);
    item_lines(&mut laid, ids[1], [(0.0, 80.0), (80.0, 180.0)]);
    let grows = growths(&b, &laid, 0.0, 100.0, 100.0);
    assert_eq!(grows.len(), 1, "{grows:?}");
    let Pushed::Line(line) = &grows[0].target else {
        panic!("line growth expected: {grows:?}");
    };
    assert_eq!((line.line, grows[0].at, grows[0].amount), (1, 80.0, 20.0));
}

#[test]
fn orphans_and_widows_of_an_item_do_not_move_its_row() {
    // The kept pair of lines starts at the item's top: honouring it would need the
    // row moved, which only the first piece and break-inside: avoid do.
    let (b, ids, mut laid) = row_of_two(90.0);
    let lines = [(0.0, 60.0), (60.0, 90.0)];
    item_lines(&mut laid, ids[1], lines);
    let flow = laid.flows.get_mut(&ids[1]).unwrap();
    flow.keep_lines(2, 2);
    assert_eq!(flow.solid, vec![(0.0, 90.0)]);
    laid.content.insert(
        ids[1],
        lines
            .iter()
            .map(
                |&(top, bottom)| crate::layout::fragment::flow::ContentLine {
                    top,
                    bottom,
                    ..Default::default()
                },
            )
            .collect(),
    );
    let m = b.breaks(90.0, &laid).finish(FlowGeometry::default(), 90.0);
    assert!(m.keep.is_empty(), "{:?}", m.keep);
    let grows = growths(&b, &laid, 0.0, 70.0, 70.0);
    assert_eq!(grows.len(), 1, "{grows:?}");
    assert_eq!((grows[0].at, grows[0].amount), (60.0, 10.0));
}

#[test]
fn monoliths_inside_an_item_stay_unbreakable_for_the_grid() {
    let (b, ids) = grid(
        "<div style='display:grid'><div><div style='contain:size;height:150px'></div><div style='contain:size;height:150px'></div></div></div>",
    );
    let item = &b.grid.as_ref().unwrap().items[0];
    let kids: Vec<u64> = item
        .inner
        .as_ref()
        .unwrap()
        .kids
        .iter()
        .map(|k| k.id)
        .collect();
    let mut laid = Laid::of_boxes(&[
        (b.id, 0.0, 300.0),
        (ids[0], 0.0, 300.0),
        (kids[0], 0.0, 150.0),
        (kids[1], 150.0, 300.0),
    ]);
    laid.grids.insert(b.id, vec![(0.0, 300.0)]);
    laid.placements.insert(ids[0], (0, 0));
    let m = b
        .breaks(300.0, &laid)
        .finish(FlowGeometry::default(), 300.0);
    assert!(m.solid.contains(&(0.0, 150.0)), "{:?}", m.solid);
    assert!(m.solid.contains(&(150.0, 300.0)), "{:?}", m.solid);
}

#[test]
fn a_row_after_an_empty_row_is_pushed() {
    // Row 1 holds no in-flow item (only an out-of-flow box): a forced break before
    // row 2 still moves the row and every row after it.
    let (b, ids) = grid(
        "<div style='display:grid;grid-template-rows:50px 100px'><div style='position:absolute'></div><div style='grid-row-start:2'></div></div>",
    );
    assert_eq!(ids.len(), 1);
    let mut laid = Laid::of_boxes(&[(b.id, 0.0, 150.0), (ids[0], 50.0, 100.0)]);
    laid.grids.insert(b.id, vec![(0.0, 50.0), (50.0, 150.0)]);
    laid.placements.insert(ids[0], (1, 1));
    assert_eq!(
        b.pushed_at(50.0, &laid),
        Some(Pushed::Row(RowPush {
            grid: b.id,
            track: 0,
        }))
    );
}

#[test]
fn item_lines_out_of_flow_tables_and_flex_take_the_native_path() {
    for body in [
        "<div style='line-height:0'><span style='display:inline-block;height:60px'></span></div>",
        "<div>text</div><div style='position:absolute'></div>",
        "<div><table><tr><td>cell</td></tr></table></div>",
        "<div style='display:flex'><div>a</div></div>",
    ] {
        let doc = crate::dom::parse(&format!("<div style='display:grid'>{body}</div>"), "");
        assert!(grid_tree(first_div(&doc).unwrap()), "{body}");
    }
    let doc = crate::dom::parse(
        "<div style='display:grid'><div style='columns:2'>a</div></div>",
        "",
    );
    assert!(!grid_tree(first_div(&doc).unwrap()));
}
