//! Счёт строк в BFC clamp-контейнера (css-overflow-4 §max-lines): ровно N строк без хвоста,
//! N+1, строки независимых и вложенных контекстов, коробка между строкой и точкой среза.

use super::ClampEntry;
use super::geometry::ClampGeometry;
use gpui::{Bounds, point, px, size};
use std::collections::HashMap;

/// Paragraph probe `seq` of `lines` lines of 10px at `y`.
fn paragraph(seq: u32, y: f32, lines: u32) -> ClampEntry {
    ClampEntry {
        bounds: Bounds::new(point(px(0.), px(y)), size(px(100.), px(10. * lines as f32))),
        line: 10.,
        skip_count: false,
        fixed_height: false,
        bp_after: 0.,
        end_margin: 0.,
        min_end: 0.,
        capped: false,
        node: 0,
        seq: Some(seq),
        clamped: None,
        empty: false,
        span: None,
        bt_before: 0.,
        positioned: false,
        float: false,
    }
}

/// Box probe of an independent formatting context root from `y0` to `y1`.
fn independent_box(node: u64, y0: f32, y1: f32) -> ClampEntry {
    ClampEntry {
        bounds: Bounds::new(point(px(0.), px(y0)), size(px(100.), px(y1 - y0))),
        line: 0.,
        skip_count: true,
        fixed_height: true,
        node,
        seq: None,
        ..paragraph(0, 0., 0)
    }
}

fn counted(limit: u32) -> ClampGeometry {
    ClampGeometry {
        limit: Some(limit),
        max_h: None,
        trim_end: 0.,
    }
}

#[test]
fn exactly_n_lines_without_following_content_have_no_clamp_point() {
    let plan = counted(3).compute(vec![paragraph(0, 0., 3)], &HashMap::new(), 0.);
    assert_eq!(plan.para, None);
    assert_eq!(plan.cut, f32::INFINITY);
}

#[test]
fn n_plus_one_lines_clamp_after_the_nth_line() {
    let plan = counted(3).compute(vec![paragraph(0, 0., 4)], &HashMap::new(), 0.);
    assert_eq!(plan.para, Some((0, 3)));
    // The paragraph shortens itself: no box is cut.
    assert_eq!(plan.cut, f32::INFINITY);
    let plan = counted(3).compute(
        vec![paragraph(0, 0., 3), paragraph(1, 30., 1)],
        &HashMap::new(),
        0.,
    );
    assert_eq!((plan.para, plan.cut), (Some((0, 3)), 30.));
}

#[test]
fn lines_of_independent_and_nested_contexts_are_not_counted() {
    // A flow-root with a nested flow-root: five lines, none counted.
    let mut outer = paragraph(0, 0., 2);
    outer.skip_count = true;
    let mut nested = paragraph(1, 20., 3);
    nested.skip_count = true;
    let entries = vec![
        independent_box(7, 0., 50.),
        outer,
        independent_box(8, 20., 50.),
        nested,
        paragraph(2, 50., 3),
    ];
    let plan = counted(2).compute(entries, &HashMap::new(), 0.);
    assert_eq!(plan.para, Some((2, 2)));
    assert_eq!(plan.cut, f32::INFINITY);
}

#[test]
fn an_in_flow_box_between_the_line_and_the_clamp_point_suppresses_the_ellipsis() {
    let automatic = ClampGeometry {
        limit: None,
        max_h: Some(65.),
        trim_end: 0.,
    };
    // Line 1, a multicol (its lines are laid out by the columns), Line 2.
    let entries = vec![
        paragraph(0, 0., 2),
        independent_box(7, 20., 60.),
        paragraph(2, 60., 2),
    ];
    let plan = automatic.compute(entries.clone(), &HashMap::new(), 0.);
    assert_eq!(plan.para, None);
    assert_eq!(plan.next, Some(2));
    // A float is out of flow: the line still precedes the clamp point.
    let mut floated = entries;
    floated[1].float = true;
    floated[1].skip_count = false;
    floated[1].fixed_height = false;
    let plan = automatic.compute(floated, &HashMap::new(), 0.);
    assert_eq!(plan.para, Some((0, 2)));
}
