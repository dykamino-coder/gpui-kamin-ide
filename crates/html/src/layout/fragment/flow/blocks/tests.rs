//! Тесты точек разрыва блочной стопки по заданной геометрии border box.

use super::{BlockBox, Laid, Pushed};
use crate::dom::Node;
use crate::layout::fragment::flow::FlowGeometry;

fn tree(html: &str) -> (BlockBox, Vec<u64>) {
    let doc = crate::dom::parse(html, "");
    // The first `div` of the parsed document (under `html` and `body`).
    fn div(n: &Node) -> Option<&crate::dom::Element> {
        let Node::Element(e) = n else { return None };
        if e.tag == "div" {
            return Some(e);
        }
        e.children.iter().find_map(div)
    }
    let root = doc.iter().find_map(div).expect("block root");
    let b = BlockBox::collect(
        root,
        crate::layout::fragment::ShapeCx::COLUMNS,
        doc.values(),
    );
    // Without the root: the root is not recorded and stands at zero.
    let ids = b.ids()[1..].to_vec();
    (b, ids)
}

fn at(boxes: &[(u64, f32, f32)]) -> Laid {
    Laid::of_boxes(boxes)
}

type Out = (Vec<(f32, f32)>, Vec<f32>, Vec<(f32, f32)>);

fn finish(b: &BlockBox, end: f32, boxes: &Laid) -> Out {
    let m = b.breaks(end, boxes).finish(FlowGeometry::default(), end);
    (m.cuts, m.forced, m.solid)
}

#[test]
fn sibling_boundary_truncates_both_margins() {
    // Margins 30 collapse between the boxes: the cut keeps the end of the
    // first border box and resumes at the top of the second (css-break-3 §5.2).
    let (b, ids) = tree(
        "<div><div style='height:40px;margin-bottom:30px'></div><div style='height:40px;margin-top:30px'></div></div>",
    );
    let boxes = at(&[(ids[0], 0.0, 40.0), (ids[1], 70.0, 110.0)]);
    let (cuts, forced, solid) = finish(&b, 110.0, &boxes);
    assert_eq!(cuts, vec![(40.0, 70.0)]);
    assert!(forced.is_empty());
    assert!(solid.is_empty());
}

#[test]
fn forced_break_lands_on_the_sibling_boundary() {
    let (b, ids) = tree(
        "<div><div style='height:40px'></div><div style='height:40px;break-before:column'></div></div>",
    );
    let boxes = at(&[(ids[0], 0.0, 40.0), (ids[1], 40.0, 80.0)]);
    let (cuts, forced, _) = finish(&b, 80.0, &boxes);
    assert_eq!(cuts, vec![(40.0, 40.0)]);
    assert_eq!(forced, vec![40.0]);
}

#[test]
fn avoided_boundary_holds_from_the_last_break_inside_the_previous_box() {
    // `break-before: avoid` closes the boundary between b and c: the range runs
    // from the last class A point inside the previous wrapper (between a and
    // b) to the start of c (css-break-4 §4.3 rule 1; Blink EarlyBreak).
    let (b, ids) = tree(
        "<div><div style='height:10px'></div><div><div id=a style='height:30px'></div><div id=b style='height:30px'></div></div><div style='break-before:avoid;height:30px'></div></div>",
    );
    // ids: top0, wrap, a, b, c (pre-order)
    let boxes = at(&[
        (ids[0], 0.0, 10.0),
        (ids[1], 10.0, 70.0),
        (ids[2], 10.0, 40.0),
        (ids[3], 40.0, 70.0),
        (ids[4], 70.0, 100.0),
    ]);
    let (cuts, _, solid) = finish(&b, 100.0, &boxes);
    assert_eq!(cuts, vec![(10.0, 10.0), (40.0, 40.0)]);
    assert_eq!(solid, vec![(40.0, 70.05)]);
}

#[test]
fn monoliths_and_block_start_border_are_unbreakable() {
    let (b, ids) = tree(
        "<div><div style='border-top:5px solid;padding-top:5px'><div style='height:20px'></div></div><div style='height:50px;contain:size'></div></div>",
    );
    let boxes = at(&[
        (ids[0], 0.0, 30.0),
        (ids[1], 10.0, 30.0),
        (ids[2], 30.0, 80.0),
    ]);
    let (cuts, _, solid) = finish(&b, 80.0, &boxes);
    assert_eq!(cuts, vec![(30.0, 30.0)]);
    assert!(solid.contains(&(0.0, 10.0)));
    assert!(solid.contains(&(30.0, 80.0)));
}

#[test]
fn inline_content_between_blocks_is_not_a_sibling_boundary() {
    let (b, ids) =
        tree("<div><div style='height:10px'></div>text<div style='height:10px'></div></div>");
    let boxes = at(&[(ids[0], 0.0, 10.0), (ids[1], 30.0, 40.0)]);
    let (cuts, _, _) = finish(&b, 40.0, &boxes);
    assert!(cuts.is_empty());
}

#[test]
fn unrecorded_box_drops_its_boundaries() {
    let (b, ids) = tree(
        "<div><div style='height:10px'></div><div style='height:10px'></div><div style='height:10px'></div></div>",
    );
    let boxes = at(&[(ids[0], 0.0, 10.0), (ids[2], 20.0, 30.0)]);
    let (cuts, _, _) = finish(&b, 30.0, &boxes);
    assert!(cuts.is_empty());
}

#[test]
fn trailing_margin_of_a_formatting_context_root_is_truncated() {
    // The last child's end margin stays inside a BFC root (CSS 2.1 §10.6.7)
    // and is truncated by a break after the child (css-break-3 §5.2).
    let (b, ids) = tree(
        "<div><div style='overflow:hidden'><div style='height:50px;margin-bottom:50px'></div></div></div>",
    );
    let boxes = at(&[(ids[0], 0.0, 100.0), (ids[1], 0.0, 50.0)]);
    let (cuts, _, _) = finish(&b, 100.0, &boxes);
    assert_eq!(cuts, vec![(50.0, 100.0)]);
}

#[test]
fn block_end_padding_holds_the_adjoining_line() {
    // No class C break between the last line and the end padding without a
    // gap (css-break-4 §possible-breaks): the line goes with the padding.
    let (b, _) = tree("<div style='padding-bottom:40px'>a</div>");
    let mut lines = FlowGeometry::lines([30.0, 30.0]);
    lines.normalize();
    let m = b.breaks(100.0, &Laid::default()).finish(lines, 100.0);
    assert!(m.solid.contains(&(30.0, 100.0)));
    assert!(m.cuts.contains(&(30.0, 30.0)));
}

#[test]
fn block_inside_inline_is_a_sibling_of_the_flow_blocks() {
    // CSS 2.1 §9.2.1.1: the block splits the inline box and stands in the
    // same block flow; the margin between the two blocks is truncated.
    let (b, ids) = tree(
        "<div><div style='height:70px'></div><span><div style='margin-top:160px;height:50px'></div></span></div>",
    );
    let boxes = at(&[(ids[0], 0.0, 70.0), (ids[1], 230.0, 280.0)]);
    let (cuts, _, _) = finish(&b, 280.0, &boxes);
    assert_eq!(cuts, vec![(70.0, 230.0)]);
}

#[test]
fn pushed_box_is_the_outermost_box_starting_at_the_point() {
    // The wrapper and its first child start together (collapsed margin): the
    // growth strut goes before the wrapper, as the stack plan cuts there.
    let (b, ids) = tree(
        "<div><div style='height:40px'></div><div><div style='height:60px;break-inside:avoid'></div></div></div>",
    );
    let boxes = at(&[
        (ids[0], 0.0, 40.0),
        (ids[1], 40.0, 100.0),
        (ids[2], 40.0, 100.0),
    ]);
    assert_eq!(b.pushed_at(40.0, &boxes), Some(Pushed::Box(ids[1])));
}

#[test]
fn pushed_box_descends_into_the_box_covering_the_point() {
    let (b, ids) = tree(
        "<div><div style='padding-top:10px'><div style='height:30px'></div><div style='height:60px'></div></div></div>",
    );
    let boxes = at(&[
        (ids[0], 0.0, 100.0),
        (ids[1], 10.0, 40.0),
        (ids[2], 40.0, 100.0),
    ]);
    let found = |a: f32| b.pushed_at(a, &boxes);
    assert_eq!(found(40.0), Some(Pushed::Box(ids[2])));
    // No box starts inside the second child: no strut.
    assert_eq!(found(70.0), None);
}

#[test]
fn root_avoid_after_page_padding_does_not_create_a_blank_fragment() {
    // css-break-4 §4.4: на пустом листе корневой avoid ослабляется;
    // вложенная коробка avoid всё ещё переносится целиком.
    let (b, ids) = tree(
        "<div style='break-inside:avoid'><div style='height:96px'></div><div style='break-inside:avoid;height:192px'></div></div>",
    );
    let laid = at(&[
        (b.id, 8.0, 296.0),
        (ids[0], 8.0, 104.0),
        (ids[1], 104.0, 296.0),
    ]);
    let m = b
        .breaks(296.0, &laid)
        .finish(FlowGeometry::default(), 296.0);
    let k = crate::layout::fragment::types::Kid {
        h: 296.0,
        over: 296.0,
        monolith: true,
        cuts: m.cuts,
        solid: m.solid,
        keep: m.keep,
        ..Default::default()
    };
    let (_, _, plan) = crate::layout::fragment::plan::fill(
        &[k],
        192.0,
        crate::layout::fragment::plan::Space::pages(4),
    );
    assert_eq!(plan.len(), 2);
    assert_eq!((plan[0].col, plan[0].from, plan[0].h), (0, 0.0, 104.0));
    assert_eq!((plan[1].col, plan[1].from, plan[1].h), (1, 104.0, 192.0));
}
