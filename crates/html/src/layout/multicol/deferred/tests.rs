//! Отложенная раскладка сохраняет процентный CB при определении ширины колонки.
use super::*;
use crate::layout::fragment::types::StackAxis;
use crate::style::values::value::Len;
use std::cell::RefCell;

pub(super) fn section(nodes: &[Node]) -> &Element {
    fn find(nodes: &[Node]) -> Option<&Element> {
        nodes.iter().find_map(|n| match n {
            Node::Element(e) if e.tag == "section" => Some(e),
            Node::Element(e) => find(&e.children),
            _ => None,
        })
    }
    find(nodes).expect("section")
}

#[test]
fn deferred_children_resolve_percentages_from_original_content_box() {
    let tree = crate::dom::parse(
        "<!doctype html><section style='height:400px;box-sizing:border-box;padding:20px;border:10px solid;column-count:2'><div style='height:50%;min-height:25%;max-height:75%'></div></section>",
        "",
    );
    let host = section(&tree);
    let mut opts = RenderOpts {
        text: Default::default(),
        viewport: (800.0, 600.0),
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    };
    opts.cx.values.doc = crate::layout::cx::BuildDoc(tree.doc.clone());
    opts.cx.values.viewport = opts.viewport;
    crate::style::cascade::materialize::materialize(
        &tree,
        opts.root_style().style_rc(),
        opts.cx.vcx(),
    );
    let merged = crate::style::cascade::materialize::box_style(
        &opts.root_style(),
        &host.style,
        opts.cx.vcx(),
    );
    let recipe = Recipe::new(
        host,
        &merged,
        &opts,
        2,
        16.0,
        Some(100.0),
        StackAxis::Horizontal,
        None,
    )
    .expect("deferred measured tree");
    for width in [100.0, 160.0] {
        let resolved = recipe.resolved_host(width);
        let Node::Element(child) = &resolved.children[0] else {
            unreachable!()
        };
        assert_eq!(child.style.used().height, Some(Len::Px(170.0)));
        let measured = RefCell::default();
        let kids = laid::children(&resolved, &merged, opts.cx.vcx(), width, &measured)
            .expect("resolved measured tree");
        let child = kids[0].0.style.used();
        assert_eq!(child.height, Some(Len::Px(170.0)));
        assert_eq!(child.min_height, Some(Len::Px(85.0)));
        assert_eq!(child.max_height, Some(Len::Px(255.0)));
    }
    let Node::Element(original) = &host.children[0] else {
        unreachable!()
    };
    assert_eq!(original.style.used().height, Some(Len::Pct(0.5)));
}

#[gpui::test]
fn deferred_width_preserves_the_full_width_float_measurement_contract(
    cx: &mut gpui::TestAppContext,
) {
    let cx = cx.add_empty_window();
    let tree = crate::dom::parse(
        "<!doctype html><section style='column-count:2;height:150px'><div style='height:100px'></div><div style='float:left;break-inside:avoid;width:100%;height:100px'><div style='height:60px'></div></div><div style='float:left;width:100%;height:0'><div style='height:40px;margin-top:-40px'></div></div></section>",
        "",
    );
    let host = section(&tree);
    let mut opts = RenderOpts {
        text: Default::default(),
        viewport: (800.0, 600.0),
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    };
    opts.cx.values.doc = crate::layout::cx::BuildDoc(tree.doc.clone());
    opts.cx.values.viewport = opts.viewport;
    crate::style::cascade::materialize::materialize(
        &tree,
        opts.root_style().style_rc(),
        opts.cx.vcx(),
    );
    let merged = crate::style::cascade::materialize::box_style(
        &opts.root_style(),
        &host.style,
        opts.cx.vcx(),
    );
    let recipe = Recipe::new(
        host,
        &merged,
        &opts,
        2,
        0.0,
        Some(150.0),
        StackAxis::Horizontal,
        None,
    )
    .expect("pending geometry must have a deferred recipe");
    for width in [50.0, 80.0] {
        let resolved = recipe.resolved_host(width);
        let Node::Element(float) = &resolved.children[1] else {
            panic!("float")
        };
        assert!(float.marks.float_block);
        assert_eq!(float.style.used().float, None);
        assert_eq!(float.style.used().height, Some(Len::Px(100.0)));
        cx.draw(
            gpui::point(gpui::px(0.), gpui::px(0.)),
            gpui::size(gpui::px(400.), gpui::px(400.)),
            |window, app| {
                assert_eq!(recipe.recorded_children(width, window, app).0.len(), 3);
                gpui::div()
            },
        );
    }
    let Node::Element(original) = &host.children[1] else {
        panic!("original float")
    };
    assert_eq!(original.style.used().float, Some(-1));
}
