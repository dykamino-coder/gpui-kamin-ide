//! Тесты `fragment_size`.

use super::*;
use crate::dom::Node;

fn constrained(
    natural: f32,
    style: &Computed,
    decoration: f32,
    viewport: Option<(f32, f32)>,
    overflow_extent: bool,
) -> f32 {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    constrain(
        natural,
        style,
        decoration,
        viewport,
        overflow_extent,
        doc.values(),
    )
}

fn element(style: Computed, children: Vec<Node>) -> Element {
    Element {
        marks: Default::default(),
        list_item: None,
        node_id: 0,
        anim: None,
        tag: "div".into(),
        style: style.into(),
        hover: None,
        first_letter: None,
        first_line: None,
        children,
        attrs: Vec::new(),
        inline: false,
    }
}

#[test]
fn positioned_descendants_keep_their_balancing_extent() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    use crate::style::computed::Position;
    let child = element(
        Computed {
            height: Some(Len::Px(160.0)),
            position: Some(Position::Absolute),
            ..Default::default()
        },
        Vec::new(),
    );
    let parent = element(
        Computed {
            height: Some(Len::Px(100.0)),
            position: Some(Position::Relative),
            ..Default::default()
        },
        vec![Node::Element(child)],
    );
    assert_eq!(
        shape_full(&parent, 4, ShapeCx::COLUMNS, vcx)
            .unwrap()
            .height,
        160.0
    );
}

#[test]
fn measured_box_and_parallel_flow_keep_distinct_sizes_and_break_ranges() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let child = element(
        Computed {
            height: Some(Len::Px(206.0)),
            ..Default::default()
        },
        Vec::new(),
    );
    let parent = element(
        Computed {
            min_height: Some(Len::Px(120.0)),
            max_height: Some(Len::Px(160.0)),
            ..Default::default()
        },
        vec![Node::Element(child)],
    );
    let used = shape_full(&parent, 4, ShapeCx::COLUMNS, vcx).unwrap();
    let overflow = shape_full(
        &parent,
        4,
        ShapeCx {
            unclamped: true,
            ..ShapeCx::COLUMNS
        },
        vcx,
    )
    .unwrap();
    assert_eq!(used.height, 160.0);
    assert_eq!(overflow.height, 206.0);
    assert!(used.solid.iter().all(|(_, end)| *end <= used.height));
}

#[test]
fn visible_overflow_does_not_remove_the_box_maximum() {
    let style = Computed {
        min_height: Some(Len::Px(120.0)),
        max_height: Some(Len::Px(160.0)),
        ..Default::default()
    };
    assert_eq!(constrained(206.0, &style, 0.0, None, false), 160.0);
    assert_eq!(constrained(206.0, &style, 0.0, None, true), 206.0);
}

#[test]
fn minimum_wins_over_maximum_in_both_flow_modes() {
    let style = Computed {
        min_height: Some(Len::Px(200.0)),
        max_height: Some(Len::Px(100.0)),
        overflow_y: Some(Overflow::Clip),
        ..Default::default()
    };
    assert_eq!(constrained(300.0, &style, 0.0, None, false), 200.0);
    assert_eq!(constrained(300.0, &style, 0.0, None, true), 200.0);
}

#[test]
fn border_box_constraints_include_decoration_once() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let measure = |style: Computed| {
        shape_full(&element(style, Vec::new()), 4, ShapeCx::COLUMNS, vcx)
            .unwrap()
            .height
    };
    let mut style = Computed {
        height: Some(Len::Px(40.0)),
        min_height: Some(Len::Px(30.0)),
        max_height: Some(Len::Px(50.0)),
        border_box: Some(true),
        padding: crate::style::computed::Sides {
            top: Some(Len::Px(10.0)),
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(measure(style.clone()), 40.0);
    style.border_box = Some(false);
    assert_eq!(measure(style.clone()), 50.0);
    style.height = Some(Len::Px(2.0));
    style.min_height = None;
    style.border_box = Some(true);
    assert_eq!(measure(style), 10.0);
}

#[test]
fn viewport_constraints_use_the_actual_page_area() {
    let style = Computed {
        height: Some(Len::Vh(0.5)),
        max_height: Some(Len::Vw(0.25)),
        ..Default::default()
    };
    assert_eq!(
        constrained(400.0, &style, 0.0, Some((800.0, 600.0)), false),
        200.0
    );
    assert_eq!(constrained(400.0, &style, 0.0, None, false), 400.0);
}

#[cfg(feature = "wpt-harness")]
#[test]
fn estimate_probe_records_the_shape_caller_without_double_counting() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let probe = crate::FragmentProbes::default();
    let cx = ShapeCx {
        probe: Some(&probe),
        ..ShapeCx::COLUMNS
    };
    let root = element(Computed::default(), Vec::new());
    let call_line = line!() + 1;
    assert!(shape_full(&root, 4, cx, doc.values()).is_some());
    let counters = probe.take();
    let measured = &counters["columns.estimate"];
    assert_eq!(measured.estimate_calls, 1);
    assert_eq!(
        measured.estimate_sites,
        std::collections::BTreeMap::from([(format!("{}:{call_line}", file!()), 1)])
    );
}
