//! Геометрия разрывов основного и параллельных потоков.
use super::*;

#[test]
fn clearance_does_not_consume_a_parallel_float() {
    for (top, margins, resume) in [
        (10., [0., 0.], 10.),
        (40., [10., 0.], 30.),
        (10., [-20., -20.], 10.),
    ] {
        let geometry = FlowGeometry {
            cuts: vec![(10., 250.)],
            floats: vec![FloatGeometry {
                id: 1,
                anchor: top - margins[0],
                top,
                bottom: 250.,
                margins,
                first_break: Some(top),
            }],
            ..Default::default()
        };
        let breaks = blocks::BlockBreaks::default().finish(geometry, 260.);
        assert_eq!(breaks.cuts, vec![(10., resume)]);
        // Float остаётся параллельным: его высота не запрещает разрыв
        // обычного потока и не превращается в монолит.
        assert!(breaks.solid.is_empty());
    }
}

#[test]
fn nested_probe_preserves_the_original_line_growth_address() {
    let growth = std::rc::Rc::new(std::cell::RefCell::new(vec![0.0; 3]));
    let source = FlowGeometry {
        boxes: [(7, (10.0, 25.0))].into(),
        line_pushes: vec![LinePush {
            at: 20.0,
            line: 2,
            growth: growth.clone(),
        }],
        ..Default::default()
    };
    let mut geometry = FlowGeometry::default();
    geometry.append(&source, 30.0);
    drop(source);
    assert_eq!(geometry.line_pushes[0].at, 50.0);
    assert_eq!(
        super::blocks::Laid::default().with_flow(&geometry).at(7),
        Some((40.0, 55.0))
    );
    assert!(geometry.line_pushes[0].grow(10.0));
    assert_eq!(*growth.borrow(), vec![0.0, 0.0, 10.0]);
}

#[test]
fn positioned_band_slots_replace_probe_coordinates_before_break_collection() {
    let laid = blocks::Laid::of_boxes(&[(1, 0., 576.), (2, 483., 915.), (3, 915., 1059.)]);
    let geometry = FlowGeometry {
        boxes: [(2, (0., 432.)), (3, (432., 576.))].into(),
        ..Default::default()
    };
    let laid = laid.with_flow(&geometry);
    let root = blocks::BlockBox {
        id: 1,
        kids: vec![
            blocks::BlockKid {
                id: 2,
                monolith: true,
                ..Default::default()
            },
            blocks::BlockKid {
                id: 3,
                adjoins: true,
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    assert_eq!(flow_end(&root, &laid), Some(576.));
    let breaks = root
        .breaks(576., &laid)
        .finish(FlowGeometry::default(), 576.);
    assert_eq!(breaks.cuts, [(432., 432.)]);
    assert_eq!(breaks.barriers, [(0., 432.)]);
}

#[test]
fn parallel_lines_only_break_at_shared_edges() {
    let mut geometry = FlowGeometry::lines([10.0, 20.0]);
    geometry.append(&FlowGeometry::lines([15.0, 15.0]), 0.0);
    geometry.normalize();
    assert_eq!(geometry.cuts, vec![(30.0, 30.0)]);
}

#[test]
fn orphans_and_widows_keep_edge_lines_together() {
    let mut geometry = FlowGeometry::lines([10.0; 5]);
    geometry.keep_lines(2, 2);
    assert_eq!(
        geometry.solid,
        vec![(0.0, 20.0), (20.0, 30.0), (30.0, 50.0)]
    );
    assert_eq!(
        geometry.cuts,
        vec![(20.0, 20.0), (30.0, 30.0), (50.0, 50.0)]
    );
    // Too few lines: the paragraph is one range, broken only around it.
    let mut geometry = FlowGeometry::lines([10.0; 3]);
    geometry.keep_lines(2, 2);
    assert_eq!(geometry.solid, vec![(0.0, 30.0)]);
    assert_eq!(geometry.cuts, vec![(30.0, 30.0)]);
}

#[test]
fn float_translation_preserves_unbreakable_margins() {
    let source = FlowGeometry {
        floats: vec![FloatGeometry {
            id: 7,
            anchor: 0.0,
            top: 40.0,
            bottom: 60.0,
            margins: [40.0, 40.0],
            first_break: Some(50.0),
        }],
        ..Default::default()
    };
    let mut geometry = FlowGeometry::default();
    geometry.append(&source, 15.0);
    let g = geometry.floats[0];
    assert_eq!((g.id, g.top, g.bottom), (7, 55.0, 75.0));
    assert_eq!(g.first_break, Some(65.0));
    assert_eq!(g.anchor, 0.0);
    assert_eq!(
        (g.top - g.margins[0], g.bottom + g.margins[1]),
        (15.0, 115.0)
    );
}

#[test]
fn translated_rows_include_inline_box_growth() {
    let mut geometry = FlowGeometry::default();
    geometry.append(&FlowGeometry::lines([10.0, 30.0, 10.0]), 5.0);
    geometry.normalize();
    assert_eq!(
        geometry.cuts,
        vec![(15.0, 15.0), (45.0, 45.0), (55.0, 55.0)]
    );
    assert_eq!(geometry.solid[1], (15.0, 45.0));
}

#[test]
fn table_wrapper_preserves_caption_and_cell_line_geometry() {
    let tree = crate::dom::parse(
        "<table><caption>caption</caption><tr><td>cell</td></tr></table>",
        "",
    );
    fn find(nodes: &[crate::dom::Node]) -> Option<&crate::dom::Element> {
        nodes.iter().find_map(|n| match n {
            crate::dom::Node::Element(e) if e.tag == "table" => Some(e),
            crate::dom::Node::Element(e) => find(&e.children),
            _ => None,
        })
    }
    let table = find(&tree).unwrap();
    assert!(flow_lines(table));
    let mut atomic = table.clone();
    atomic.style.used_mut().display = Some(crate::style::computed::Display::InlineTable);
    assert!(!flow_lines(&atomic));
}
