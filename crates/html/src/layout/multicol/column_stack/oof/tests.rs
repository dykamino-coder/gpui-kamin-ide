//! Контракт адаптера: Space и физическая коробка берутся из одного реального ряда.

use super::*;
use crate::layout::fragment::types::{Rows, StackAxis};
use gpui::{point, px, size};

fn stack(axis: StackAxis, wrap: bool) -> ColumnStack {
    let stack = ColumnStack::new(
        Vec::new(),
        2,
        10.,
        None,
        None,
        wrap.then_some(Rows {
            h: None,
            gap: 13.,
            wrap,
            cap: false,
        }),
        None,
        None,
        Default::default(),
    )
    .with_axis(axis);
    stack.col_w.set(40.);
    *stack.lines_plan.borrow_mut() = vec![(0., 70.), (83., 35.)];
    stack
}

#[test]
fn wrapped_columns_use_each_actual_row_height() {
    let stack = stack(StackAxis::Horizontal, true);
    let bounds = Bounds {
        origin: point(px(11.), px(17.)),
        size: size(px(90.), px(118.)),
    };
    let first = stack.fragmentainer(1, bounds).unwrap();
    let tail = stack.fragmentainer(2, bounds).unwrap();
    assert_eq!(first.space.block_size, Some(70.));
    assert_eq!(tail.space.block_size, Some(35.));
    assert_eq!(
        first.rect,
        Bounds {
            origin: point(px(61.), px(17.)),
            size: size(px(40.), px(70.))
        }
    );
    assert_eq!(
        tail.rect,
        Bounds {
            origin: point(px(11.), px(100.)),
            size: size(px(40.), px(35.))
        }
    );
    assert!(matches!(
        stack.fragmentainer(4, bounds),
        Err(Error::InvalidGeometry)
    ));
}

#[test]
fn overflow_columns_keep_the_existing_line_without_balancing_it() {
    let stack = stack(StackAxis::Horizontal, false);
    let actual = stack
        .fragmentainer(
            5,
            Bounds {
                origin: point(px(0.), px(0.)),
                size: size(px(90.), px(70.)),
            },
        )
        .unwrap();
    assert_eq!(actual.space.block_size, Some(70.));
    assert_eq!(actual.rect.origin.x, px(250.));
}

#[test]
fn vertical_rl_uses_the_physical_right_edge() {
    let stack = stack(StackAxis::VerticalRl, true);
    let actual = stack
        .fragmentainer(
            2,
            Bounds {
                origin: point(px(11.), px(17.)),
                size: size(px(118.), px(90.)),
            },
        )
        .unwrap();
    assert_eq!(actual.space.inline_size, Some(40.));
    assert_eq!(actual.space.block_size, Some(35.));
    assert_eq!(
        actual.rect,
        Bounds {
            origin: point(px(11.), px(17.)),
            size: size(px(35.), px(40.))
        }
    );
}
