//! Проверки адресного бюджета многоколоночных рядов.

use super::row_plan;
use crate::dom::{Element, Node};
use crate::layout::fragment::token::{Space, space::sequence::Sequence};
use crate::layout::options::RenderOpts;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

fn host(nodes: &[Node]) -> &Element {
    fn find(nodes: &[Node]) -> Option<&Element> {
        nodes.iter().find_map(|node| match node {
            Node::Element(e) if e.tag == "div" => Some(e),
            Node::Element(e) => find(&e.children),
            _ => None,
        })
    }
    find(nodes).expect("host")
}

#[test]
fn addressed_budget_keeps_first_remainder_and_later_constraints() {
    let tree = crate::dom::parse("<div style='height:500px; min-height:600px'></div>", "");
    let e = host(&tree);
    let opts = RenderOpts {
        text: Default::default(),
        viewport: (800., 600.),
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    };
    let sequence = Sequence::new(|index| {
        Ok(Space {
            block_size: Some(if index == 0 { 100. } else { 40. }),
            ..Space::pages()
        })
    })
    .nested(0, 75.)
    .unwrap();
    let plan = row_plan(
        e,
        &Computed::default(),
        &opts,
        None,
        2,
        false,
        Some(sequence.clone()),
    );
    let outer = plan.outer_space.unwrap();
    assert_eq!(outer.at(0).unwrap().block_size, Some(25.));
    assert_eq!(outer.at(1).unwrap().block_size, Some(40.));
    assert_eq!((plan.nest_rows, plan.nest_phase), (Some(100.), 75.));
    assert_eq!(e.style.used().min_height, Some(Len::Px(600.)));
    for property in ["column-height:30px", "column-wrap:nowrap"] {
        let tree = crate::dom::parse(&format!("<div style='{property}'></div>"), "");
        let plan = row_plan(
            host(&tree),
            &Computed::default(),
            &opts,
            None,
            2,
            false,
            Some(sequence.clone()),
        );
        assert!(plan.outer_space.is_none());
    }
}

#[test]
fn measured_rows_and_auto_prefix_share_constraints_without_mutating_host() {
    use crate::layout::multicol::dimensions::MulticolDimensions;
    let tree = crate::dom::parse(
        "<div style='height:500px; max-height:90px; min-height:60px'></div>",
        "",
    );
    let e = host(&tree);
    let opts = RenderOpts {
        text: Default::default(),
        viewport: (800., 600.),
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    };
    let input = MulticolDimensions::measured(&e.style.used(), 200., Some(80.));
    let plan = super::row_plan_in(e, &opts, None, 2, false, None, input);
    assert_eq!(plan.box_h, Some(80.));
    assert_eq!(plan.rows.unwrap().h, Some(80.));
    let prefix = super::row_plan_in(e, &opts, None, 2, false, None, input.prefix());
    assert_eq!(prefix.box_h, None);
    assert!(prefix.rows.is_none());
    assert_eq!(e.style.used().height, Some(Len::Px(500.)));
}

#[test]
fn measured_rows_keep_indefinite_own_height_and_relative_minimum() {
    use crate::layout::multicol::dimensions::MulticolDimensions;
    let e = Element::synthetic(
        "div",
        Computed {
            height: Some(Len::Pct(0.5)),
            min_height: Some(Len::Em(2.)),
            ..Default::default()
        },
        Default::default(),
        vec![],
    );
    let opts = RenderOpts {
        text: Default::default(),
        viewport: (800., 600.),
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    };
    for height in [None, Some(80.)] {
        let own = MulticolDimensions::measured(&e.style.used(), 200., height);
        let plan = super::row_plan_in(&e, &opts, None, 2, false, None, own);
        assert_eq!(plan.box_h, height);
        assert!(plan.rows.is_none());
    }
    assert_eq!(e.style.used().height, Some(Len::Pct(0.5)));
    assert_eq!(e.style.used().min_height, Some(Len::Em(2.)));
}
