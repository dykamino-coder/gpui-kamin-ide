//! Независимый float minimum не закрывает точки normal flow.
use super::*;
use crate::layout::fragment::flow::FloatGeometry;

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
    let stack = ColumnStack::new(
        vec![],
        2,
        0.0,
        None,
        None,
        None,
        None,
        None,
        Default::default(),
    );
    let (height, _) = stack.balance_line(std::slice::from_ref(&kid), 8, None);
    assert_eq!(height, 100.0);
    let (cols, _, plan) = plan::fill(&[kid], 20.0, Space::columns(8));
    assert_eq!(cols, 6);
    assert_eq!(plan[0].h, 20.0);
    assert_eq!(plan[1].from, 20.0);
}
