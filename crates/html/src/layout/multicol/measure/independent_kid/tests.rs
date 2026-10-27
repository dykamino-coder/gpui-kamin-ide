//! Мера раскладкой копии: сетка, flex, таблица и обёртка вложенных колонок.

use super::*;

#[gpui::test]
fn empty_grid_uses_measured_copy_at_known_and_deferred_widths(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    use crate::dom::Node;
    use crate::layout::fragment::types::StackAxis;
    use crate::layout::multicol::deferred::Recipe;
    use crate::layout::options::RenderOpts;
    let tree = crate::dom::parse(
        "<div><div style='display:grid;grid-template-rows:20px 30px'></div></div>",
        "",
    );
    fn find(nodes: &[Node]) -> Option<&Element> {
        nodes.iter().find_map(|n| match n {
            Node::Element(e) if e.tag == "div" => Some(e),
            Node::Element(e) => find(&e.children),
            _ => None,
        })
    }
    let opts = RenderOpts {
        text: Default::default(),
        viewport: (800.0, 600.0),
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    };
    let host = find(&tree).unwrap();
    let recipe = Recipe::new(
        host,
        &BoxStyle::default(),
        &opts,
        2,
        0.0,
        Some(100.0),
        StackAxis::Horizontal,
        None,
    )
    .unwrap();
    for width in [100.0, 120.0] {
        cx.draw(
            gpui::point(gpui::px(0.), gpui::px(0.)),
            gpui::size(gpui::px(400.), gpui::px(400.)),
            |window, app| {
                let kids = recipe.children(width, window, app);
                assert_eq!(kids.len(), 1);
                assert_eq!(kids[0].measure, Some(width));
                let Node::Element(grid) = &host.children[0] else {
                    panic!("grid")
                };
                assert!(gate(grid, true, Some(width)));
                assert!(kids[0].watched.as_ref().is_some_and(|w| w.has_blocks()));
                gpui::div()
            },
        );
    }
    #[cfg(feature = "wpt-harness")]
    assert!(
        opts.frag
            .probe
            .take()
            .values()
            .all(|p| p.estimate_calls == 0)
    );
}

#[test]
fn percent_margins_use_the_copy_inline_size() {
    let tree = crate::dom::parse(
        "<div style='display:grid;margin-top:10%;margin-bottom:5%'><div>x</div></div>",
        "",
    );
    fn find(nodes: &[crate::dom::Node]) -> Option<&Element> {
        nodes.iter().find_map(|n| match n {
            crate::dom::Node::Element(e) if e.tag == "div" => Some(e),
            crate::dom::Node::Element(e) => find(&e.children),
            _ => None,
        })
    }
    let c = find(&tree).expect("authored grid");
    let registry = RefCell::default();
    let (_, shape) =
        measured(c, &BoxStyle::default(), tree.values(), 120.0, &registry).expect("copy");
    assert_eq!((shape.margin_top, shape.margin_bottom), (12.0, 6.0));
    assert_eq!(shape.height, 0.0);
    assert_eq!(*registry.borrow(), vec![(c.node_id, 120.0)]);
}

#[test]
fn table_rows_and_cells_come_from_the_laid_copy() {
    use crate::dom::Node;
    use crate::layout::multicol::measure::measure_children;
    use crate::layout::options::RenderOpts;

    // An out-of-flow sibling keeps the stack off the all-native path; the
    // table still gets its rows, spans and cell content from layout.
    let tree = crate::dom::parse(
        "<div><div style='position:absolute'></div><table style='border-spacing:0'>\
         <tr><td style='break-inside:avoid'><div style='height:50px'></div></td></tr>\
         <tr><td rowspan=2><div style='height:70px'></div></td></tr><tr></tr></table></div>",
        "",
    );
    fn host(nodes: &[Node]) -> Option<&Element> {
        nodes.iter().find_map(|n| match n {
            Node::Element(e) if e.tag == "div" => Some(e),
            Node::Element(e) => host(&e.children),
            _ => None,
        })
    }
    let host = host(&tree).expect("host");
    let table = host
        .children
        .iter()
        .find_map(|n| match n {
            Node::Element(e) if e.tag == "table" => Some(e),
            _ => None,
        })
        .expect("table");
    assert!(gate(table, true, Some(50.0)));
    assert!(
        !gate(table, false, Some(50.0)),
        "no layout measure in this stack"
    );
    let opts = RenderOpts {
        text: Default::default(),
        viewport: (800.0, 600.0),
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    };
    let measured: RefCell<Vec<(u64, f32)>> = RefCell::default();
    for (whole_ok, measure_ok) in [(true, false), (false, true)] {
        measured.borrow_mut().clear();
        let kids = measure_children(
            host,
            &BoxStyle::default(),
            &opts,
            None,
            None,
            None,
            (Some(50.0), Some(50.0)),
            &RefCell::default(),
            whole_ok,
            &RefCell::default(),
            &measured,
            false,
            false,
            measure_ok,
            true,
        )
        .expect("table child");
        assert_eq!(kids.len(), 1);
        assert_eq!(kids[0].1.height, 0.0, "height comes from layout");
        assert!(kids[0].1.cuts.is_empty() && kids[0].1.solid.is_empty());
        assert_eq!(*measured.borrow(), vec![(table.node_id, 50.0)]);
    }
    #[cfg(feature = "wpt-harness")]
    assert!(
        opts.frag
            .probe
            .take()
            .values()
            .all(|p| p.estimate_calls == 0)
    );
}

#[test]
fn nested_columns_preserve_the_roots_fragmentation_dispatch() {
    use crate::dom::Node;
    use crate::layout::multicol::measure::measure_children;
    use crate::layout::options::RenderOpts;

    fn host(nodes: &[Node]) -> Option<&Element> {
        nodes.iter().find_map(|n| match n {
            Node::Element(e) if e.tag == "div" => Some(e),
            Node::Element(e) => host(&e.children),
            _ => None,
        })
    }
    for (style, independent, paginate) in [
        ("", true, false),
        ("display:block", true, false),
        ("display:list-item", true, false),
        ("display:flex;flex-direction:column", false, false),
        ("display:flex;flex-direction:row", false, false),
        ("display:grid", false, false),
        ("display:flex;flex-direction:column", false, true),
        ("display:flex;flex-direction:row", false, true),
    ] {
        // Анонимный flex-элемент не входит в flex_tree; атомарные колонки
        // внутри соседнего блока допускают DOM-пагинацию этого flex.
        let (before, after, columns_display, tag) = if paginate {
            ("anonymous<div>", "</div>", "display:inline-block;", "span")
        } else {
            ("", "", "", "div")
        };
        let html = format!(
            "<div><div style='{style}'>{before}\
             <{tag} style='{columns_display}columns:2;column-gap:0'>\
             <div style='height:30px'></div><div style='height:50px'></div>\
             </{tag}>{after}</div></div>"
        );
        let tree = crate::dom::parse(&html, "");
        let host = host(&tree).expect("authored host");
        let Node::Element(root) = &host.children[0] else {
            panic!("formatting root")
        };
        assert_eq!(gate(root, true, Some(80.0)), independent, "{style}");
        assert_eq!(
            own_layout_gate(root, true, Some(80.0)),
            independent,
            "{style}"
        );
        let opts = RenderOpts {
            text: Default::default(),
            viewport: (800.0, 600.0),
            normal_line_height: 1.2,
            cx: Default::default(),
            frag: Default::default(),
        };
        let measured = RefCell::default();
        let kids = measure_children(
            host,
            &BoxStyle::default(),
            &opts,
            None,
            None,
            None,
            (Some(80.0), Some(80.0)),
            &RefCell::default(),
            false,
            &RefCell::default(),
            &measured,
            false,
            false,
            true,
            true,
        )
        .expect("supported fragmentation subtree");
        assert_eq!(kids.len(), 1);
        assert_eq!(kids[0].0.marks.paginate, paginate, "{style}");
        if !independent && !paginate {
            assert!(measured.borrow().is_empty(), "specialized shape planner");
            assert!(
                kids[0].1.height > 0.0,
                "flex/grid retains fragmentation geometry"
            );
        } else {
            assert_eq!(*measured.borrow(), vec![(root.node_id, 80.0)], "{style}");
            assert_eq!(kids[0].1.height, 0.0, "height comes from layout");
        }
    }
}
