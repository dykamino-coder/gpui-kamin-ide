//! Ряды резервируют виды после измерения, в том числе у пустого блока.

use crate::dom::{Element, Node};
use crate::layout::fragment::types::{Rows, StackAxis};
use crate::layout::multicol::{column_stack::ColumnStack, deferred::Recipe};
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;

fn host(nodes: &[Node]) -> &Element {
    fn find(nodes: &[Node]) -> Option<&Element> {
        nodes.iter().find_map(|n| match n {
            Node::Element(e) if e.tag == "div" => Some(e),
            Node::Element(e) => find(&e.children),
            _ => None,
        })
    }
    find(nodes).expect("authored div")
}

#[test]
fn rows_reserve_all_child_views_from_measured_overflow() {
    for (axis, mode) in [
        (StackAxis::Horizontal, "horizontal-tb"),
        (StackAxis::VerticalLr, "vertical-lr"),
        (StackAxis::VerticalRl, "vertical-rl"),
    ] {
        reserve_views(axis, mode);
    }
}

fn reserve_views(axis: StackAxis, mode: &str) {
    let tree = crate::dom::parse(
        &format!(
            "<div><div style='writing-mode:{mode}'></div><div style='writing-mode:{mode}'>text</div></div>"
        ),
        "",
    );
    let opts = RenderOpts {
        text: Default::default(),
        viewport: (800.0, 600.0),
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    };
    let rows = Rows {
        h: Some(100.0),
        gap: 10.0,
        wrap: true,
        cap: false,
    };
    let recipe = Recipe::new(
        host(&tree),
        &BoxStyle::default(),
        &opts,
        2,
        0.0,
        Some(100.0),
        axis,
        Some(rows),
    )
    .expect("independent row recipe");
    let children = recipe.children(100.0);
    assert_eq!(children.len(), 2);
    assert!(children.iter().all(|c| c.regrow.is_some()));
    assert!(children.iter().all(|c| c.measure == Some(100.0)));
    let mut stack = ColumnStack::new(
        children,
        2,
        0.0,
        Some(100.0),
        None,
        Some(rows),
        None,
        None,
        Default::default(),
    );
    // Результат native layout: пустой корень имеет 137 px border box,
    // а видимое переполнение достигает 701 px.
    stack.children[0].h = 137.0;
    stack.children[0].over = 701.0;
    stack.children[1].h = 20.0;
    stack.children[1].over = 20.0;
    stack.reserve_row_copies();
    assert_eq!(stack.copies, 11);
    assert!(
        stack
            .children
            .iter()
            .all(|c| c.frags.len() + 1 == stack.copies)
    );
    // Поздний рост перестраивает виды из актуального DOM того же ребёнка.
    stack.children[0].over = 1101.0;
    stack.reserve_row_copies();
    assert_eq!(stack.copies, 15);
    assert!(
        stack
            .children
            .iter()
            .all(|c| c.frags.len() + 1 == stack.copies)
    );
    #[cfg(feature = "wpt-harness")]
    assert!(
        opts.frag
            .probe
            .take()
            .values()
            .all(|p| p.estimate_calls == 0)
    );
}

#[cfg(feature = "wpt-harness")]
#[test]
fn flex_stack_children_bypass_shape_and_direct_measure_calls() {
    for direction in ["row", "row-reverse", "column", "column-reverse"] {
        for wrap in ["nowrap", "wrap", "wrap-reverse"] {
            let tree = crate::dom::parse(
                &format!(
                    "<div><div style='display:flex;flex-direction:{direction};flex-wrap:{wrap}'><div style='width:70px;height:20px;order:2;break-after:avoid'></div><div style='width:70px;height:30px;order:-1;break-before:column'></div></div></div>"
                ),
                "",
            );
            let opts = RenderOpts {
                text: Default::default(),
                viewport: (800.0, 600.0),
                normal_line_height: 1.2,
                cx: Default::default(),
                frag: Default::default(),
            };
            let recipe = Recipe::new(
                host(&tree),
                &BoxStyle::default(),
                &opts,
                2,
                0.0,
                Some(100.0),
                StackAxis::Horizontal,
                None,
            )
            .expect("normal-flow flex recipe");
            let children = recipe.children(100.0);
            assert_eq!(children.len(), 1, "{direction}/{wrap}");
            assert_eq!(children[0].measure, Some(100.0));
            assert!(children[0].regrow.is_some());
            assert!(
                opts.frag
                    .probe
                    .take()
                    .values()
                    .all(|p| p.estimate_calls == 0),
                "shape_full or direct measure reached for {direction}/{wrap}"
            );
        }
    }
}
