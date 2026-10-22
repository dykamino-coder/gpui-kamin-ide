//! Общий dispatch StackPlan: native группы не достигают остаточной оценки F2.

use super::stack_plan;
use crate::dom::{Element, Node};
use crate::layout::fragment::types::Rows;
use crate::layout::options::RenderOpts;
use crate::style::computed::Computed;
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
                "<div style='display:flex;flex-direction:row;flex-wrap:wrap'><div style='width:70px;height:50px'>first</div><div style='width:70px;height:50px'>second</div></div>",
                "<div style='display:flex;flex-direction:row-reverse;flex-wrap:wrap-reverse'><div style='width:70px;height:50px'></div><div style='width:70px;height:50px'></div></div>",
                "<div style='display:flex;flex-direction:column;flex-wrap:wrap'><div style='width:70px;height:50px'></div><div style='width:70px;height:50px'></div></div>",
                "<div style='display:flex;flex-direction:column-reverse;flex-wrap:wrap-reverse'><div style='width:70px;height:50px'></div><div style='width:70px;height:50px'></div></div>",
                "<div style='margin:calc(10% + 3px);height:20px;width:30px'>text</div>",
                "<div style='display:grid;grid-template-rows:auto 1fr 10%'></div>",
                "<div style='display:grid;height:150px;width:150px;align-content:center;grid-template-rows:20px 30px'></div>",
                "<div style='display:grid'><div style='height:40px;width:30px'></div></div>",
                "<table><tr><td><div style='height:20px;width:30px'></div></td></tr></table>",
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
                    e.style.writing_mode = Some(wm);
                    e.style.flat_mode = Some(wm);
                    e.style.rtl = Some(rtl);
                    for n in &mut e.children {
                        if let Node::Element(k) = n {
                            writing(k, mode, rtl);
                        }
                    }
                }
                writing(&mut root, mode, rtl);
                for auto in [false, true] {
                    root.style.column_fill_auto = Some(auto);
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
                            opts.cx.doc = crate::layout::cx::BuildDoc(tree.doc.clone());
                            let plan = stack_plan(
                                &root,
                                &root.style,
                                &Computed::default(),
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
                            let kids = plan.stackable.expect("native group");
                            assert!(!kids.is_empty(), "{mode}: {body}");
                            assert!(kids.iter().all(|(_, shape)| shape.height == 0.0));
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
fn cloned_decoration_is_visible_as_an_explicit_f2_1_estimate() {
    // ADR P-19 F2-1: clone зависит от края и consumed, native группы выше — нет.
    let tree = crate::dom::parse(
        "<div><div style='box-decoration-break:clone;padding:1px;height:40px'></div></div>",
        "",
    );
    let root = host(&tree);
    let opts = opts();
    let _ = stack_plan(
        root,
        &root.style,
        &Computed::default(),
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
    assert!(
        opts.frag
            .probe
            .take()
            .values()
            .any(|p| p.estimate_calls > 0)
    );
}
