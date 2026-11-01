//! Устойчивая числовая проба сохраняет измеренное дерево и рекордеры.

use super::*;
use crate::dom::{Element, Node};
use crate::layout::multicol::deferred::Recipe;
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use std::{cell::Cell, rc::Rc};

fn host(nodes: &[Node]) -> &Element {
    fn find(nodes: &[Node]) -> Option<&Element> {
        nodes.iter().find_map(|node| match node {
            Node::Element(e) if e.tag == "section" => Some(e),
            Node::Element(e) => find(&e.children),
            _ => None,
        })
    }
    find(nodes).expect("section")
}

#[gpui::test]
fn stable_measured_children_are_not_rebuilt(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    for contents in [
        "<div style='height:20px;break-inside:avoid'></div>".repeat(12),
        "<div style='height:20px;break-before:column'></div>".repeat(3),
        "<div style='height:40px;break-inside:avoid'><span style='float:left;width:10px;height:30px'></span></div>".repeat(8),
    ] {
        let tree = crate::document::parsed::parse(&format!("<section>{contents}</section>"), "");
        let opts = RenderOpts {
            text: Default::default(), viewport: (800., 600.), normal_line_height: 1.2,
            cx: Default::default(), frag: Default::default(),
        };
        let recipe = Recipe::new(host(&tree), &BoxStyle::default(), &opts, 4, None,
            crate::layout::fragment::types::StackAxis::Horizontal, None).unwrap();
        cx.draw(gpui::point(gpui::px(0.), gpui::px(0.)),
            gpui::size(gpui::px(800.), gpui::px(600.)), |window, app| {
                let children = recipe.children(100., window, app);
                let mut stack = ColumnStack::new(children, 4, 0., None, None, None, None, Default::default());
                for index in 0..stack.children.len() {
                    stack.lay_out_main(index, window, app);
                }
                let builds = Rc::new(Cell::new(0));
                let watches: Vec<_> = stack.children.iter().map(|c| c.watched.clone()).collect();
                let before = format!("{:?}", stack.kid_geoms());
                for regrow in stack.children.iter_mut().filter_map(|c| c.regrow.as_mut()) {
                    let original = regrow.build.clone();
                    let builds = builds.clone();
                    regrow.build = Rc::new(move |dom, part, band, opts| {
                        builds.set(builds.get() + 1);
                        original(dom, part, band, opts)
                    });
                }
                assert!(stack.children.iter().any(|c| c.regrow.is_some()));
                stack.regrow_children(window, app);
                assert!(stack.balance_failure.is_none());
                assert_eq!(builds.get(), 0);
                assert_eq!(format!("{:?}", stack.kid_geoms()), before);
                for (child, watch) in stack.children.iter().zip(watches) {
                    if let (Some(current), Some(original)) = (&child.watched, watch) {
                        assert!(Rc::ptr_eq(current, &original));
                    }
                }
                gpui::div()
            });
    }
}
