//! Независимый float minimum не закрывает точки normal flow.
use super::*;
use crate::layout::fragment::flow::FloatGeometry;

#[test]
fn balancing_keeps_an_avoided_boundary_with_an_earlier_break() {
    let kids: Vec<_> = [25., 50., 25., 50.]
        .into_iter()
        .enumerate()
        .map(|(i, h)| Kid {
            h,
            over: h,
            avoid_before: i == 2,
            ..Default::default()
        })
        .collect();
    let stack = ColumnStack::new(vec![], 2, 0., None, None, None, None, Default::default());
    let (height, _) = stack.balance_line(&kids, 2, None);
    assert_eq!(height, 100.);
}

#[test]
fn float_balance_floor_preserves_independent_normal_breaks() {
    let float = FloatGeometry {
        id: 7,
        anchor: 0.0,
        top: 40.0,
        bottom: 60.0,
        margins: [40.0, 40.0],
        first_break: None,
    };
    let kid = Kid {
        h: 120.0,
        over: 120.0,
        cuts: (1..6).map(|i| (i as f32 * 20.0, i as f32 * 20.0)).collect(),
        float_min: float.balance_min(),
        ..Default::default()
    };
    let stack = ColumnStack::new(vec![], 2, 0.0, None, None, None, None, Default::default());
    let (height, _) = stack.balance_line(std::slice::from_ref(&kid), 8, None);
    assert_eq!(height, 100.0);
    let (cols, _, plan) = plan::fill_at(&[kid], &|_| 20.0, Space::columns(8));
    assert_eq!(cols, 6);
    assert_eq!(plan[0].h, 20.0);
    assert_eq!(plan[1].from, 20.0);
}

#[test]
fn forced_groups_can_require_more_than_forty_eight_columns() {
    let kids: Vec<_> = (0..65)
        .map(|i| Kid {
            h: 20.,
            over: 20.,
            monolith: true,
            force_before: i > 0,
            ..Default::default()
        })
        .collect();
    let mut stack = ColumnStack::new(
        vec![],
        2,
        0.,
        None,
        None,
        None,
        None,
        None,
        Default::default(),
    );
    let (height, plan) = stack.balance_line(&kids, 2, None);
    assert_eq!(height, 20.);
    assert_eq!(plan.completion, plan::Completion::Complete);
    assert_eq!(plan.last().unwrap().col, 64);
    stack.forced_h = Some(height);
    let replay = stack.balance_line(&kids, 2, None).1;
    assert_eq!(replay.len(), 65);
    assert_eq!(replay.last().unwrap().col, 64);
}
