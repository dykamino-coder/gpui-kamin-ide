//! Геометрия разрывов основного и параллельных потоков.
use super::*;

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
