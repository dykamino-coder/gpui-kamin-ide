//! Представленческие атрибуты не меняют уже каскадированную геометрию таблиц.

use super::*;
use crate::dom::Node;

fn table(nodes: &[Node]) -> Option<&Element> {
    nodes.iter().find_map(|node| match node {
        Node::Element(e) if e.tag == "table" => Some(e),
        Node::Element(e) => table(&e.children),
        _ => None,
    })
}

#[test]
fn author_padding_and_spacing_match_attribute_free_fragmentation() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    for paged in [false, true] {
        for span in ["", " rowspan=2"] {
            for css in [
                "table{border-spacing:0}td{padding:1px}",
                "table{border-spacing:0}td{padding:1px 4px 3px 2px}",
                "table{border-spacing:2px}td{padding:0}",
            ] {
                let html = |attrs: &str| {
                    format!(
                        "<!doctype html><style>{css}div{{height:10px}}</style><table {attrs}><tr><td{span}><div></div></td><td><div></div></td></tr><tr><td><div></div></td></tr></table>"
                    )
                };
                let reference = crate::dom::parse(&html(""), "");
                let hinted = crate::dom::parse(&html("cellpadding=10 cellspacing=10"), "");
                let cx = ShapeCx {
                    paged,
                    ..ShapeCx::COLUMNS
                };
                let expected =
                    table_shape(table(&reference).unwrap(), 8, cx, doc.values()).unwrap();
                let actual = table_shape(table(&hinted).unwrap(), 8, cx, doc.values()).unwrap();
                assert_eq!(
                    actual.height, expected.height,
                    "{css}, {span}, paged={paged}"
                );
                assert_eq!(actual.cuts, expected.cuts);
                assert_eq!(actual.forced, expected.forced);
                assert_eq!(actual.solid, expected.solid);
            }
        }
    }
}
