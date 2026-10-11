//! Тесты полос занятости флоатов (FloatBands).

use super::*;
use crate::layout::float::shapes::FloatShape;

/// Правило 1: левый флоат встаёт вплотную к левому краю.
#[test]
fn left_hugs_edge() {
    let mut b = FloatBands::new(200.0);
    assert_eq!(b.add_float(-1, 50.0, 20.0, None), (0.0, 0.0));
    assert_eq!(b.available(0.0, 10.0), (50.0, 200.0));
    // Ниже флоата строка снова во всю ширину.
    assert_eq!(b.available(20.0, 10.0), (0.0, 200.0));
}

/// Правило 1 для правой стороны.
#[test]
fn right_hugs_edge() {
    let mut b = FloatBands::new(200.0);
    assert_eq!(b.add_float(1, 50.0, 20.0, None), (150.0, 0.0));
    assert_eq!(b.available(0.0, 10.0), (0.0, 150.0));
}

/// Правило 3: два левых флоата встают в ряд, третий не влезает и уходит
/// вниз — та самая лесенка (правило 2), своего кода не имеющая.
#[test]
fn stair_step() {
    let mut b = FloatBands::new(100.0);
    assert_eq!(b.add_float(-1, 60.0, 10.0, None), (0.0, 0.0));
    // 60 + 60 > 100 — второй под первый.
    assert_eq!(b.add_float(-1, 60.0, 10.0, None), (0.0, 10.0));
}

/// Правило 3: левый и правый на одной полосе, пока хватает ширины.
#[test]
fn left_and_right_share_band() {
    let mut b = FloatBands::new(100.0);
    assert_eq!(b.add_float(-1, 40.0, 10.0, None), (0.0, 0.0));
    assert_eq!(b.add_float(1, 40.0, 10.0, None), (60.0, 0.0));
    assert_eq!(b.available(0.0, 5.0), (40.0, 60.0));
    // Третий не влезает между ними.
    assert_eq!(b.add_float(-1, 40.0, 10.0, None), (0.0, 10.0));
}

/// Правило 7: одинокий слишком широкий флоат вылезает, а не уезжает вниз.
#[test]
fn too_wide_alone_overflows() {
    let mut b = FloatBands::new(50.0);
    assert_eq!(b.add_float(-1, 80.0, 10.0, None), (0.0, 0.0));
}

/// Правило 5: новый флоат не выше верха более раннего.
#[test]
fn ceiling_never_rises() {
    let mut b = FloatBands::new(100.0);
    b.add_float(-1, 100.0, 10.0, None);
    // Второй влезть рядом не может — уходит под первый и остаётся там,
    // даже если третий узкий.
    assert_eq!(b.add_float(-1, 10.0, 10.0, None), (0.0, 10.0));
    assert_eq!(b.add_float(-1, 10.0, 10.0, None), (10.0, 10.0));
}

/// §9.5.2: `clear: left` проходит мимо правого флоата.
#[test]
fn clear_is_sided() {
    let mut b = FloatBands::new(100.0);
    b.add_float(1, 30.0, 40.0, None);
    assert_eq!(b.clearance(Some(-1), 0.0), 0.0);
    assert_eq!(b.clearance(Some(1), 0.0), 40.0);
    assert_eq!(b.clearance(Some(0), 0.0), 40.0);
}

/// Флоат нулевой высоты занятости не создаёт, но `clear` об него
/// срабатывает.
#[test]
fn zero_height_float_still_clears() {
    let mut b = FloatBands::new(100.0);
    b.add_float(-1, 30.0, 0.0, None);
    assert_eq!(b.available(0.0, 10.0), (0.0, 100.0));
    assert_eq!(b.bottom(Some(-1)), 0.0);
}

/// §10.6.7: низ флоатов — вклад в высоту корня контекста.
#[test]
fn bottom_tracks_sides() {
    let mut b = FloatBands::new(100.0);
    b.add_float(-1, 10.0, 25.0, None);
    b.add_float(1, 10.0, 40.0, None);
    assert_eq!(b.bottom(Some(-1)), 25.0);
    assert_eq!(b.bottom(Some(1)), 40.0);
    assert_eq!(b.bottom(None), 40.0);
}

/// Формы для набора строк: экстент считается от своей стороны.
#[test]
fn shapes_are_side_relative() {
    let mut b = FloatBands::new(100.0);
    b.add_float(-1, 30.0, 20.0, None);
    b.add_float(1, 40.0, 20.0, None);
    let (l, r) = &*b.shapes(0.0);
    assert_eq!(
        l.as_slice(),
        &[FloatShape::Band {
            top: 0.0,
            h: 20.0,
            w: 30.0
        }]
    );
    assert_eq!(
        r.as_slice(),
        &[FloatShape::Band {
            top: 0.0,
            h: 20.0,
            w: 40.0
        }]
    );
}

/// Лесенка из четырёх флоатов — геометрия `CSS2/floats-clear/floats-005`
/// (дюйм = 96 точек, содержащий блок 1.25in).
///
/// Последняя строка ловит наследование экстента при разрезе: полоса
/// [96, 120) несёт 91.2, а [120, 192) — 72.
#[test]
fn ladder() {
    let mut b = FloatBands::new(120.0);
    assert_eq!(b.add_float(-1, 115.2, 96.0, None), (0.0, 0.0));
    assert_eq!(b.add_float(-1, 72.0, 96.0, None), (0.0, 96.0));
    assert_eq!(b.add_float(-1, 19.2, 24.0, None), (72.0, 96.0));
    assert_eq!(b.add_float(-1, 48.0, 24.0, None), (72.0, 120.0));
}

/// Правило 3: узкий правый не влезает рядом с широким левым и уходит под
/// него (`floats/floats-rule3-outside-left-002`).
#[test]
fn rule3_opposite() {
    let mut b = FloatBands::new(500.0);
    assert_eq!(b.add_float(-1, 475.0, 50.0, None), (0.0, 0.0));
    assert_eq!(b.add_float(1, 50.0, 50.0, None), (450.0, 50.0));
}

/// Разрез посередине чужого флоата: новая полоса наследует его экстент.
#[test]
fn split_inherits() {
    let mut b = FloatBands::new(200.0);
    b.add_float(-1, 40.0, 100.0, None);
    // Правый встаёт на своей стороне и режет левую полосу пополам.
    assert_eq!(b.add_float(1, 30.0, 50.0, None), (170.0, 0.0));
    assert_eq!(b.available(0.0, 10.0), (40.0, 170.0));
    // Ниже правого левый ещё держится.
    assert_eq!(b.available(60.0, 10.0), (40.0, 200.0));
}

/// `available` через границу двух полос берёт худшее из обеих.
#[test]
fn available_window() {
    let mut b = FloatBands::new(200.0);
    b.add_float(-1, 30.0, 20.0, None);
    b.add_float(1, 50.0, 100.0, None);
    // Окно [10, 40) задевает и полосу с левым флоатом, и следующую.
    assert_eq!(b.available(10.0, 30.0), (30.0, 150.0));
}

/// Смещение потребителя вжигается в верх формы.
#[test]
fn shapes_shift_by_consumer_top() {
    let mut b = FloatBands::new(100.0);
    b.add_float(-1, 30.0, 50.0, None);
    let (l, _) = &*b.shapes(20.0);
    assert_eq!(
        l.as_slice(),
        &[FloatShape::Band {
            top: -20.0,
            h: 50.0,
            w: 30.0
        }]
    );
}

/// §9.5, последний абзац: коробка со своим контекстом ищет ОКНО на всю
/// свою высоту. Геометрия `CSS2/floats/floats-wrap-top-below-001l`.
#[test]
fn place_among_001l() {
    let mut b = FloatBands::new(400.0);
    assert_eq!(b.add_float(-1, 50.0, 75.0, Some(-1)), (0.0, 0.0));
    // `clear: left` сажает второй флоат под первый, а не рядом.
    assert_eq!(b.add_float(-1, 100.0, 75.0, Some(-1)), (0.0, 75.0));
    assert_eq!(b.place_among(200.0, 50.0, 0.0), (50.0, 0.0, 350.0));
    assert_eq!(b.place_among(200.0, 50.0, 50.0), (100.0, 50.0, 300.0));
}

/// `floats-wrap-top-below-002l`: правый 300 рядом не влезает (правило 3)
/// и садится на 75; вторая коробка съезжает под оба флоата.
#[test]
fn place_among_002l() {
    let mut b = FloatBands::new(400.0);
    assert_eq!(b.add_float(-1, 150.0, 75.0, None), (0.0, 0.0));
    assert_eq!(b.add_float(1, 300.0, 75.0, None), (100.0, 75.0));
    assert_eq!(b.place_among(200.0, 50.0, 0.0), (150.0, 0.0, 250.0));
    assert_eq!(b.place_among(200.0, 50.0, 50.0), (0.0, 150.0, 400.0));
}

/// `floats-wrap-top-below-003l`: окно съезжает НА ОДНУ полосу, а не ниже
/// всех флоатов.
#[test]
fn place_among_003l() {
    let mut b = FloatBands::new(400.0);
    assert_eq!(b.add_float(-1, 250.0, 75.0, None), (0.0, 0.0));
    assert_eq!(b.add_float(1, 250.0, 75.0, None), (150.0, 75.0));
    assert_eq!(b.place_among(100.0, 50.0, 0.0), (250.0, 0.0, 150.0));
    assert_eq!(b.place_among(100.0, 50.0, 50.0), (0.0, 75.0, 150.0));
}
