//! Independent coordinates exercise every grid-container baseline preference.
use super::*;
use crate::compute::grid::OriginZeroLine;
use crate::{AlignItems, NodeId, geometry::Line};

type Style = crate::Style;

fn item(
    row: (i16, i16),
    column: i16,
    order: u16,
    align: AlignItems,
    first: Option<f32>,
    last: Option<f32>,
) -> GridItem {
    let mut item = GridItem::new_with_placement_style_and_order(
        NodeId::new(order as u64),
        Line {
            start: OriginZeroLine(column),
            end: OriginZeroLine(column + 1),
        },
        Line {
            start: OriginZeroLine(row.0),
            end: OriginZeroLine(row.1),
        },
        &Style::DEFAULT,
        align,
        AlignItems::STRETCH,
        order,
    );
    item.first_baseline = first;
    item.last_baseline = last;
    item.height = 100.0;
    item.y_position = 20.0;
    item
}

#[test]
fn final_baselines_exclude_sizing_margin_and_last_distance() {
    let mut child = item(
        (0, 1),
        0,
        0,
        AlignItems::LAST_BASELINE,
        Some(30.0),
        Some(90.0),
    );
    child.baseline = Some(17.0);
    assert_eq!(
        compute(&[child]),
        Baselines {
            first: Some(110.0),
            last: Some(110.0)
        }
    );
}

#[test]
fn first_sharing_precedes_last_and_available_sets() {
    let a = item(
        (0, 1),
        0,
        0,
        AlignItems::LAST_BASELINE,
        Some(15.0),
        Some(80.0),
    );
    let b = item((0, 1), 1, 1, AlignItems::BASELINE, Some(40.0), Some(70.0));
    assert_eq!(
        compute(&[a, b]),
        Baselines {
            first: Some(60.0),
            last: Some(100.0)
        }
    );
}

#[test]
fn opposite_preference_requires_the_correct_span_edge() {
    let a = item(
        (0, 2),
        0,
        0,
        AlignItems::LAST_BASELINE,
        Some(10.0),
        Some(90.0),
    );
    let b = item((0, 1), 1, 1, AlignItems::START, Some(40.0), Some(50.0));
    assert_eq!(compute(&[b, a]).first, Some(30.0));
}

#[test]
fn missing_set_skips_to_available_grid_order_then_synthesis() {
    let a = item((0, 1), 0, 3, AlignItems::START, None, None);
    let b = item((0, 1), 1, 1, AlignItems::START, Some(40.0), Some(70.0));
    assert_eq!(
        compute(&[b, a.clone()]),
        Baselines {
            first: Some(60.0),
            last: Some(90.0)
        }
    );
    assert_eq!(
        compute(&[a]),
        Baselines {
            first: Some(120.0),
            last: Some(120.0)
        }
    );
    assert_eq!(compute(&[]), Baselines::NONE);
}

#[test]
fn source_order_breaks_grid_cell_ties_without_vector_order() {
    let a = item((0, 1), 0, 1, AlignItems::START, Some(10.0), Some(60.0));
    let b = item((0, 1), 0, 0, AlignItems::START, Some(30.0), Some(90.0));
    assert_eq!(
        compute(&[a, b]),
        Baselines {
            first: Some(50.0),
            last: Some(80.0)
        }
    );
}

#[test]
fn scroll_baselines_clamp_before_parent_translation() {
    let mut a = item((0, 1), 0, 0, AlignItems::START, Some(-10.0), Some(130.0));
    a.overflow.y = crate::Overflow::Hidden;
    assert_eq!(
        compute(&[a]),
        Baselines {
            first: Some(20.0),
            last: Some(120.0)
        }
    );
}
