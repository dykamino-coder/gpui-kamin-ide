//! Общий dispatch StackPlan: native группы не достигают остаточной оценки F2.

use super::stack_plan;
use crate::dom::{Element, Node};
use crate::layout::fragment::types::Rows;
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use crate::style::values::value::Len;

fn host(nodes: &[Node]) -> &Element {
    fn find(nodes: &[Node]) -> Option<&Element> {
        nodes.iter().find_map(|n| match n {
            Node::Element(e) if e.tag == "div" => Some(e),
            Node::Element(e) => find(&e.children),
            _ => None,
        })
    }
    find(nodes).expect("authored host")
}

fn opts() -> RenderOpts {
    RenderOpts {
        text: Default::default(),
        viewport: (800.0, 600.0),
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    }
}

#[test]
fn independent_groups_bypass_estimates_before_and_after_width_is_known() {
    let deep = format!(
        "{}<div style='height:20px'>text</div>{}",
        "<div>".repeat(24),
        "</div>".repeat(24)
    );
    for rtl in [false, true] {
        for (mode, vertical, rl) in [
            ("horizontal-tb", false, false),
            ("vertical-lr", true, false),
            ("vertical-rl", true, true),
        ] {
            for body in [
                deep.as_str(),
                "<div><div style='float:left;width:70px;height:40px'></div><div style='float:right;width:70px;height:60px'></div></div>",
                "<div><div><div style='float:left;width:70px;height:40px'></div></div><div style='clear:both;width:20px;height:20px'></div></div>",
                "<div style='display:contents;font-size:20px'><div>first</div><div>second</div></div>",
                "<div><div style='display:contents'><div style='display:flex'><div>first</div><div>second</div></div></div><div style='display:none;box-decoration-break:clone;columns:2'></div></div>",
                "<div style='display:grid'><div style='display:contents'><div style='height:20px'></div><div style='height:30px'></div></div></div>",
                "<div style='display:flex'><div style='display:contents'><div>first</div><div>second</div></div></div>",
                "<div style='display:flex;flex-direction:row;flex-wrap:wrap'><div style='width:70px;height:50px'>first</div><div style='width:70px;height:50px'>second</div></div>",
                "<div style='display:flex;flex-direction:row-reverse;flex-wrap:wrap-reverse'><div style='width:70px;height:50px'></div><div style='width:70px;height:50px'></div></div>",
                "<div style='display:flex;flex-direction:column;flex-wrap:wrap'><div style='width:70px;height:50px'></div><div style='width:70px;height:50px'></div></div>",
                "<div style='display:flex;flex-direction:column-reverse;flex-wrap:wrap-reverse'><div style='width:70px;height:50px'></div><div style='width:70px;height:50px'></div></div>",
                "<div style='margin:calc(10% + 3px);height:20px;width:30px'>text</div>",
                "<div style='display:grid;grid-template-rows:auto 1fr 10%'></div>",
                "<div style='display:grid;height:150px;width:150px;align-content:center;grid-template-rows:20px 30px'></div>",
                "<div style='display:grid'><div style='height:40px;width:30px'></div></div>",
                "<table><tr><td><div style='height:20px;width:30px'></div></td></tr></table>",
                "<table><tr><td><div><div style='display:flex'><div>first</div><div>second</div></div></div></td></tr></table>",
                "<div>text</div><div style='display:grid;grid-template-rows:20px 30px'></div>",
            ] {
                // Явное письмо у детей: StackPlan получает уже унаследованный DOM.
                let tree = crate::dom::parse(&format!("<div>{body}</div>"), "");
                let mut root = host(&tree).clone();
                fn writing(e: &mut Element, mode: &str, rtl: bool) {
                    use crate::style::computed::writing_mode::WritingModeKw;
                    let wm = match mode {
                        "vertical-lr" => WritingModeKw::VerticalLr,
                        "vertical-rl" => WritingModeKw::VerticalRl,
                        _ => WritingModeKw::HorizontalTb,
                    };
                    e.style.used_mut().writing_mode = Some(wm);
                    e.style.used_mut().flat_mode = Some(wm);
                    e.style.used_mut().rtl = Some(rtl);
                    for n in &mut e.children {
                        if let Node::Element(k) = n {
                            writing(k, mode, rtl);
                        }
                    }
                }
                writing(&mut root, mode, rtl);
                for auto in [false, true] {
                    root.style.used_mut().column_fill_auto = Some(auto);
                    for width in [None, Some(Len::Px(240.0))] {
                        for rows in [
                            None,
                            Some(Rows {
                                h: Some(100.0),
                                gap: 10.0,
                                wrap: true,
                                cap: false,
                            }),
                        ] {
                            let mut opts = opts();
                            opts.cx.values.doc = crate::layout::cx::BuildDoc(tree.doc.clone());
                            let plan = stack_plan(
                                &root,
                                &BoxStyle::shared(root.style.used()),
                                &BoxStyle::default(),
                                &opts,
                                2,
                                0.0,
                                vertical,
                                rl,
                                Some(100.0),
                                Some(100.0),
                                width,
                                rows,
                                None,
                            );
                            let kids = plan.stackable.unwrap_or_else(|| panic!("{mode}: {body}"));
                            assert!(!kids.is_empty(), "{mode}: {body}");
                            assert!(
                                kids.iter().all(|(_, shape)| shape.height == 0.0),
                                "{mode}, auto={auto}, width={width:?}, rows={}: {body}",
                                rows.is_some()
                            );
                            assert_eq!(
                                plan.measured_kids.borrow().len(),
                                if width.is_some() { kids.len() } else { 0 }
                            );
                            #[cfg(feature = "wpt-harness")]
                            assert!(
                                opts.frag
                                    .probe
                                    .take()
                                    .values()
                                    .all(|p| p.estimate_calls == 0),
                                "{mode}: {body}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[cfg(feature = "wpt-harness")]
#[test]
fn clone_uses_estimates_while_fixed_height_overflow_uses_measured_continuations() {
    // Заданный normal extent не ограничивает отдельный overflow-поток.
    for body in [
        "<div><div style='box-decoration-break:clone;padding:1px;height:40px'></div></div>",
        "<div><div style='display:flex;height:50px'><div style='height:150px'></div></div></div>",
    ] {
        let tree = crate::dom::parse(body, "");
        let root = host(&tree);
        let opts = opts();
        let plan = stack_plan(
            root,
            &BoxStyle::shared(root.style.used()),
            &BoxStyle::default(),
            &opts,
            2,
            0.0,
            false,
            false,
            Some(100.0),
            Some(100.0),
            Some(Len::Px(240.0)),
            None,
            None,
        );
        if body.contains("display:flex") {
            let Node::Element(flex) = &root.children[0] else {
                panic!("flex")
            };
            assert!(crate::layout::fragment::flow::independent_copy(flex));
            assert!(
                super::laid::children(
                    root,
                    &BoxStyle::shared(root.style.used()),
                    tree.values(),
                    120.0,
                    &Default::default()
                )
                .is_some()
            );
            let kids = plan.stackable.expect("native overflow children");
            assert!(kids.iter().all(|(_, shape)| shape.height == 0.0));
            assert_eq!(plan.measured_kids.borrow().len(), kids.len());
            assert!(
                opts.frag
                    .probe
                    .take()
                    .values()
                    .all(|p| p.estimate_calls == 0)
            );
        } else {
            assert!(
                opts.frag
                    .probe
                    .take()
                    .values()
                    .any(|p| p.estimate_calls > 0),
                "{body}"
            );
        }
    }
}
