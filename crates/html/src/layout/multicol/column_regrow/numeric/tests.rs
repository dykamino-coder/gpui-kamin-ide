//! Числовые строки сверяются с действительной раскладкой тех же распорок.

use super::*;
use crate::dom::{Element, Node};
use crate::layout::multicol::deferred::Recipe;
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use std::{cell::Cell, rc::Rc};

fn section(nodes: &[Node]) -> Option<&Element> {
    nodes.iter().find_map(|n| match n {
        Node::Element(e) if e.tag == "section" => Some(e),
        Node::Element(e) => section(&e.children),
        _ => None,
    })
}

#[gpui::test]
fn numeric_line_growth_matches_layout_without_trial_builds(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    for line_height in [10., 14.41] {
        let contents = format!(
            "<section><p style='font:10px/{line_height}px Ahem;margin:0'>{}</p></section>",
            "aa aa aa aa ".repeat(12)
        );
        let tree = crate::document::parsed::parse(&contents, "");
        let opts = RenderOpts {
            text: Default::default(),
            viewport: (800., 600.),
            normal_line_height: 1.2,
            cx: Default::default(),
            frag: Default::default(),
        };
        let recipe = Recipe::new(
            section(&tree).expect("test section"),
            &BoxStyle::default(),
            &opts,
            4,
            None,
            crate::layout::fragment::types::StackAxis::Horizontal,
            None,
        )
        .unwrap();
        cx.draw(
            gpui::point(gpui::px(0.), gpui::px(0.)),
            gpui::size(gpui::px(800.), gpui::px(600.)),
            |window, app| {
                for height in [37., 61.25, 61.251, 83.] {
                    let children = recipe.children(80., window, app);
                    let mut stack = ColumnStack::new(
                        children,
                        4,
                        0.,
                        None,
                        None,
                        None,
                        None,
                        Default::default(),
                    );
                    for i in 0..stack.children.len() {
                        stack.lay_out_main(i, window, app);
                    }
                    let mut input: Vec<_> = stack
                        .children
                        .iter()
                        .map(|c| Input {
                            geometry: c.kid(),
                            laid: c.regrow.as_ref().map(|r| r.laid.clone()),
                            lines: Lines::measure(c, stack.axis, window),
                        })
                        .collect();
                    assert!(
                        input.iter().any(|i| i.lines.is_some()),
                        "eligible measured IFC: {:?}",
                        stack.kid_geoms()
                    );
                    let builds = Rc::new(Cell::new(0));
                    for regrow in stack.children.iter_mut().filter_map(|c| c.regrow.as_mut()) {
                        let original = regrow.build.clone();
                        let builds = builds.clone();
                        regrow.build = Rc::new(move |dom, part, band, opts| {
                            builds.set(builds.get() + 1);
                            original(dom, part, band, opts)
                        });
                    }
                    stack.forced_h = Some(height);
                    assert_eq!(stack.settle_numeric(&mut input), Some(Ok(())));
                    assert_eq!(builds.get(), 0, "numeric trials preserve paint children");
                    let numeric = stack.kid_geoms();
                    let expected_growth: f32 = input
                        .iter()
                        .filter_map(|i| i.lines.as_ref())
                        .flat_map(|l| &l.extra)
                        .sum();
                    assert!(
                        expected_growth > 0.,
                        "fixture must exercise dependent growth"
                    );
                    // Числовые пробы не меняют общие ограничения абзаца.
                    for lines in input.iter().filter_map(|i| i.lines.as_ref()) {
                        assert!(lines.pushes.iter().all(|p| p.growth.borrow().is_empty()));
                    }
                    stack.reset_numeric(&mut input);
                    assert!(stack.numeric_balance(height, height, true, window, app));
                    assert!(stack.balance_failure.is_none());
                    assert_eq!(builds.get(), 1, "only the chosen child is materialized");
                    let actual = stack.kid_geoms();
                    for (a, b) in numeric.iter().zip(&actual) {
                        assert!((a.h - b.h).abs() < 0.01, "{height}: {} != {}", a.h, b.h);
                        for (a, b) in [(&a.cuts, &b.cuts), (&a.solid, &b.solid), (&a.keep, &b.keep)]
                        {
                            assert_eq!(a.len(), b.len(), "{height}: {a:?} != {b:?}");
                            for (a, b) in a.iter().zip(b) {
                                assert!(
                                    (a.0 - b.0).abs() < 0.01 && (a.1 - b.1).abs() < 0.01,
                                    "{height}: {a:?} != {b:?}"
                                );
                            }
                        }
                    }
                }
                gpui::div()
            },
        );
    }
}
