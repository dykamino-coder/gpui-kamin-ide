//! Тесты-регрессии решателя измеренной пагинации.

use super::*;

fn mono(id: u64, top: f32, bottom: f32) -> Geom {
    Geom {
        id,
        cand: Cand::Mono { soft: false },
        top,
        bottom,
    }
}

#[test]
fn pushes_straddling_monolith_to_next_boundary() {
    let g = [mono(1, 50.0, 150.0)];
    assert_eq!(first_violation(&g, 100.0, 0.0).map(|v| v.1), Some(50.0));
    // Starting 20px down its first column, the boundary is at 80.
    assert_eq!(first_violation(&g, 100.0, 20.0).map(|v| v.1), Some(30.0));
}

#[test]
fn keeps_monolith_at_fragmentainer_start() {
    let g = [mono(1, 100.0, 250.0)];
    assert!(first_violation(&g, 100.0, 0.0).is_none());
}

#[test]
fn forced_break_moves_border_edge_and_keeps_margin() {
    let g = [Geom {
        id: 2,
        cand: Cand::Forced { mt: 10.0 },
        top: 70.0,
        bottom: 90.0,
    }];
    assert_eq!(first_violation(&g, 100.0, 0.0).map(|v| v.1), Some(40.0));
    let done = [Geom {
        top: 110.0,
        bottom: 130.0,
        ..g[0]
    }];
    assert!(first_violation(&done, 100.0, 0.0).is_none());
}

#[test]
fn tall_avoided_float_moves_once_then_relaxes_at_start() {
    let g = Geom {
        id: 1,
        cand: Cand::Float { soft: true },
        top: 20.0,
        bottom: 220.0,
    };
    assert_eq!(first_violation(&[g], 100.0, 0.0), Some((1, 80.0, 200.0)));
    assert!(
        first_violation(
            &[Geom {
                top: 100.0,
                bottom: 300.0,
                ..g
            }],
            100.0,
            0.0
        )
        .is_none()
    );
    assert!(
        first_violation(
            &[Geom {
                cand: Cand::Mono { soft: true },
                ..g
            }],
            100.0,
            0.0
        )
        .is_none()
    );
}

#[test]
fn avoided_edge_breaks_before_previous_box() {
    let prev = Geom {
        id: 1,
        cand: Cand::Item,
        top: 50.0,
        bottom: 100.0,
    };
    let next = Geom {
        id: 2,
        cand: Cand::Avoid { prev: 1 },
        top: 100.0,
        bottom: 400.0,
    };
    assert_eq!(
        first_violation(&[prev, next], 100.0, 0.0).map(|v| (v.0, v.1)),
        Some((1, 50.0))
    );
}
