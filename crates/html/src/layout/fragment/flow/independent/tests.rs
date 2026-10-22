//! Вертикальные метаданные сохраняют физическую копию и native ряды.

use super::*;
use crate::layout::fragment::{ShapeCx, flow::blocks::BlockBox};

fn authored(nodes: &[Node]) -> &Element {
    fn find(nodes: &[Node]) -> Option<&Element> {
        nodes.iter().find_map(|n| match n {
            Node::Element(e) if e.tag == "div" => Some(e),
            Node::Element(e) => find(&e.children),
            _ => None,
        })
    }
    find(nodes).expect("authored root")
}

#[test]
fn vertical_rows_keep_their_recorded_kind_and_original_physical_axes() {
    for (mode, rl) in [("vertical-lr", false), ("vertical-rl", true)] {
        for body in [
            "<div style='display:grid;grid-template-rows:20px 30px'><div></div><div></div></div>",
            "<table><tr><td><div></div></td></tr><tr><td></td></tr></table>",
        ] {
            let tree = crate::dom::parse(
                &format!("<div style='writing-mode:{mode};width:60px;height:100px'>{body}</div>"),
                "",
            );
            let physical = authored(&tree);
            assert!(vertical_copy(physical, rl), "{mode}: {body}");
            let logical = logical_copy(physical, rl).unwrap();
            assert!(!logical.style.wm().vertical());
            assert!(physical.style.wm().vertical());
            assert_eq!(logical.style.height, physical.style.width);
            assert_eq!(logical.style.width, physical.style.height);
            let blocks = BlockBox::collect(&logical, ShapeCx::COLUMNS, tree.values());
            if body.starts_with("<table") {
                assert_eq!(blocks.table_keys().len(), 1);
            } else {
                let mut grids = Vec::new();
                blocks.grids(&mut grids);
                assert_eq!(grids.len(), 1);
                assert_eq!(grids[0].1.len(), 2);
            }
        }
    }
}

#[test]
fn orthogonal_descendant_requires_its_own_continuation() {
    let tree = crate::dom::parse(
        "<div style='writing-mode:vertical-rl'><div style='writing-mode:horizontal-tb'></div></div>",
        "",
    );
    assert!(!vertical_copy(authored(&tree), true));
}
