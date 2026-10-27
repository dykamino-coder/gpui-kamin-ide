//! Вертикальные печатные группы сохраняют физическое дерево и native-рекордеры.

use super::tests::authored;
use super::*;
use crate::style::values::value::Len;

#[test]
fn vertical_empty_page_floats_keep_bandflow_extent_and_physical_tree() {
    for (mode, rl) in [("vertical-lr", false), ("vertical-rl", true)] {
        let tree = crate::dom::parse(
            &format!(
                "<div style='writing-mode:{mode};height:100px'><div style='float:left;width:40px;height:70px'></div><div style='float:right;width:60px;height:70px'></div></div>"
            ),
            "",
        );
        let nodes = [Node::Element(authored(&tree).clone())];
        let p = vertical_group(&nodes, &BoxStyle::default(), 120.0, 0.0, rl, tree.values())
            .expect("native empty floats");
        assert!(p.blocks && p.float_extent);
        let Node::Element(e) = &p.nodes[0] else {
            panic!("host")
        };
        assert!(e.style.used().wm().vertical());
        assert_eq!(e.style.used().height, Some(Len::Px(100.0)));
        assert_eq!(e.children.len(), 2);
    }
}

#[test]
fn vertical_page_groups_record_real_blocks_and_keep_physical_sides() {
    use crate::layout::fragment::{ShapeCx, flow::blocks::Laid};
    for (mode, rl) in [("vertical-lr", false), ("vertical-rl", true)] {
        for direction in ["ltr", "rtl"] {
            let tree = crate::dom::parse(
                &format!(
                    "<div style='writing-mode:{mode};direction:{direction};margin-left:calc(10% + 3px);margin-right:7px;padding:1px'><div style='width:20px'></div><div style='break-before:page;width:30px'></div></div>"
                ),
                "",
            );
            let nodes = [Node::Element(authored(&tree).clone())];
            for head in [0.0, 11.0] {
                let p =
                    vertical_group(&nodes, &BoxStyle::default(), 100.0, head, rl, tree.values())
                        .expect("independent vertical page group");
                let (start, end) = if rl { (7.0, 13.0) } else { (13.0, 7.0) };
                assert_eq!(
                    p.margins,
                    if head > 0.0 {
                        (0.0, end, 0.0)
                    } else {
                        (start, end, start)
                    }
                );
                let Node::Element(physical) = &p.nodes[0] else {
                    panic!("block")
                };
                assert!(physical.style.used().wm().vertical());
                assert_eq!(physical.style.used().margin.left, Some(Len::Px(13.0)));
                assert_eq!(physical.style.used().margin.right, Some(Len::Px(7.0)));
                let cx = ShapeCx {
                    paged: true,
                    ..ShapeCx::COLUMNS
                };
                let w = super::super::breaks::recorder(
                    &p.nodes,
                    &p.nodes[0],
                    Some(rl),
                    false,
                    true,
                    cx,
                    tree.values(),
                );
                let blocks = w.blocks().expect("vertical block recorder");
                assert_eq!(blocks.id, physical.node_id);
                assert_eq!(blocks.kids.len(), 2);
                let mut laid = Laid::default();
                laid.boxes.insert(blocks.id, (0.0, 52.0));
                laid.boxes.insert(blocks.kids[0].id, (1.0, 21.0));
                laid.boxes.insert(blocks.kids[1].id, (21.0, 51.0));
                let breaks = blocks.breaks(52.0, &laid);
                assert_eq!(breaks.forced, vec![21.0]);
            }
        }
    }
}

#[test]
fn vertical_page_grid_table_and_flex_use_native_recorders_without_an_estimate() {
    for (mode, rl) in [("vertical-lr", false), ("vertical-rl", true)] {
        for body in [
            "<div style='display:grid;grid-template-rows:20px 30px'></div>",
            "<table><tr><td>cell</td></tr></table>",
            "<div style='display:flex;flex-direction:row;flex-wrap:wrap'><div>first</div><div>second</div></div>",
            "<div style='display:flex;flex-direction:column-reverse;flex-wrap:wrap-reverse'><div>first</div><div>second</div></div>",
        ] {
            let tree = crate::dom::parse(
                &format!("<div style='writing-mode:{mode}'>{body}</div>"),
                "",
            );
            let nodes = [Node::Element(authored(&tree).clone())];
            let p = vertical_group(&nodes, &BoxStyle::default(), 100.0, 0.0, rl, tree.values())
                .unwrap();
            let w = super::super::breaks::recorder(
                &p.nodes,
                &p.nodes[0],
                Some(rl),
                false,
                true,
                crate::layout::fragment::ShapeCx::COLUMNS,
                tree.values(),
            );
            let blocks = w.blocks().unwrap();
            if body.starts_with("<table") {
                assert_eq!(blocks.table_keys().len(), 1);
            } else if body.contains("display:flex") {
                let mut flexes = Vec::new();
                blocks.flexes(&mut flexes);
                assert_eq!(flexes.len(), 1);
                assert_eq!(flexes[0].1.len(), 2);
            } else {
                let mut grids = Vec::new();
                blocks.grids(&mut grids);
                assert_eq!(grids.len(), 1);
                assert!(grids[0].1.is_empty());
            }
        }
    }
}
