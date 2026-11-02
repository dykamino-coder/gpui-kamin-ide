//! Числовые строки сверяются с действительной раскладкой тех же распорок.

use super::*;
use crate::dom::{Element, Node};
use crate::layout::multicol::deferred::Recipe;
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use std::{cell::Cell, rc::Rc};

mod compare;
mod rejection;

fn section(nodes: &[Node]) -> Option<&Element> {
    nodes.iter().find_map(|n| match n {
        Node::Element(e) if e.tag == "section" => Some(e),
        Node::Element(e) => section(&e.children),
        _ => None,
    })
}

#[gpui::test]
fn numeric_line_growth_matches_layout_without_trial_builds(cx: &mut gpui::TestAppContext) {
    verify(cx, 0);
}

#[gpui::test]
fn numeric_serial_growth_matches_layout_without_trial_builds(cx: &mut gpui::TestAppContext) {
    for variant in 1..=5 {
        verify(cx, variant);
    }
}

fn verify(cx: &mut gpui::TestAppContext, variant: usize) {
    let cx = cx.add_empty_window();
    for (line_height, orphans, widows) in [(10., 2, 2), (14.41, 2, 2), (14.41, 3, 4)] {
        let paragraph = format!(
            "<p style='font:10px/{line_height}px Ahem;margin:0;orphans:{orphans};widows:{widows}'>{}</p>",
            "aa aa aa aa ".repeat(12)
        );
        let column = "display:flex;flex-direction:column";
        let body = match variant {
            0 => paragraph.clone(),
            1 => format!(
                "<article style='{column}'>{}</article>",
                paragraph.repeat(3)
            ),
            2 => format!(
                "<article style='{column};row-gap:7px'><div style='{column}'>{}</div><div style='{column}'>{}</div></article>",
                paragraph.repeat(2),
                paragraph.repeat(2)
            ),
            3 => format!(
                "<article style='{column}'><div style='{column};order:2'>{paragraph}</div><div style='{column};order:-1'>{}</div></article>",
                paragraph.repeat(2)
            ),
            4 => format!(
                "<article style='{column}'>{paragraph}<div style='{column};break-before:column'>{}</div></article>",
                paragraph.repeat(2)
            ),
            _ => format!(
                "<article style='{column}'>{paragraph}<div style='{column};break-inside:avoid'>{}</div></article>",
                paragraph.repeat(2)
            ),
        };
        let contents = format!("<section>{body}</section>");
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
                            lines: Model::measure(c, stack.axis, window),
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
                    for (entry, grows) in input.iter().zip(stack.growth_requests()) {
                        if let Some(model) = &entry.lines {
                            compare::reject_mixed(model, grows);
                        }
                    }
                    assert_eq!(stack.settle_numeric(&mut input), Some(Ok(())));
                    assert_eq!(builds.get(), 0, "numeric trials preserve paint children");
                    let numeric = stack.kid_geoms();
                    let laid: Vec<_> = stack
                        .children
                        .iter()
                        .filter_map(|c| c.regrow.as_ref().map(|r| r.laid.clone()))
                        .collect();
                    let expected_growth: f32 = input
                        .iter()
                        .filter_map(|i| i.lines.as_ref())
                        .flat_map(Model::state)
                        .map(f32::from_bits)
                        .sum();
                    assert!(
                        expected_growth > 0.,
                        "fixture must exercise dependent growth"
                    );
                    // Числовые пробы не меняют общие ограничения абзаца.
                    for lines in input.iter().filter_map(|i| i.lines.as_ref()) {
                        assert!(lines.uncommitted());
                    }
                    stack.reset_numeric(&mut input);
                    assert!(stack.numeric_balance(height, height, true, window, app));
                    assert!(stack.balance_failure.is_none());
                    assert_eq!(builds.get(), 1, "only the chosen child is materialized");
                    let actual = stack.kid_geoms();
                    for (a, b) in numeric.iter().zip(&actual) {
                        compare::kid(a, b);
                    }
                    if variant != 0 {
                        for (a, b) in laid.iter().zip(
                            stack
                                .children
                                .iter()
                                .filter_map(|c| c.regrow.as_ref().map(|r| &r.laid)),
                        ) {
                            compare::laid(a, b);
                        }
                    }
                }
                gpui::div()
            },
        );
    }
}
