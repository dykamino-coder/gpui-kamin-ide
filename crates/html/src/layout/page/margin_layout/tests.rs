//! Тесты раскладки марджин-боксов листа.

use super::*;

fn auto(max: f32) -> Option<Pref> {
    Some(Pref {
        min: max,
        max,
        margins: 0.0,
        auto: true,
    })
}

#[test]
fn three_equal_boxes_share_width() {
    // `alignment-001`: по букве в каждой коробке — три равные трети.
    let s = edge_sizes([auto(8.0), auto(8.0), auto(8.0)], 450.0);
    assert!((s[0] - 150.0).abs() < 0.01 && (s[1] - 150.0).abs() < 0.01);
    assert!((s[2] - 150.0).abs() < 0.01);
}

#[test]
fn fixed_center_rest_split() {
    let fixed = Some(Pref {
        min: 100.0,
        max: 100.0,
        margins: 0.0,
        auto: false,
    });
    let s = edge_sizes([auto(10.0), fixed, auto(30.0)], 300.0);
    assert_eq!(s, [100.0, 100.0, 100.0]);
}

#[test]
fn overconstrained_moves_away_from_center() {
    // Верхняя коробка 50 при поле 100: остаток уходит в верхнее поле.
    assert_eq!(
        edge_margins(Some(0.0), Some(0.0), 50.0, 100.0, true),
        (50.0, 0.0)
    );
    assert_eq!(edge_margins(None, None, 50.0, 100.0, true), (25.0, 25.0));
}
