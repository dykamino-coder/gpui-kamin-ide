//! Тесты разбора длин и цветов.

use super::*;

#[test]
fn lengths() {
    assert_eq!(Len::parse("12px"), Some(Len::Px(12.0)));
    assert_eq!(Len::parse(" 1.5rem "), Some(Len::Px(24.0)));
    assert_eq!(Len::parse("50%"), Some(Len::Pct(0.5)));
    assert_eq!(Len::parse("auto"), Some(Len::Auto));
    // Голое число принимаем: модель часто пишет `padding: 8`.
    assert_eq!(Len::parse("8"), Some(Len::Px(8.0)));
    assert_eq!(Len::parse("нет"), None);
}

#[test]
fn colors_hex() {
    assert_eq!(
        Color::parse("#fff"),
        Some(Color {
            r: 1.,
            g: 1.,
            b: 1.,
            a: 1.
        })
    );
    let c = Color::parse("#8ab4f8").unwrap();
    assert!((c.r - 0.541).abs() < 0.01 && (c.b - 0.972).abs() < 0.01);
    assert_eq!(
        Color::parse("#00000080").map(|c| (c.a * 100.).round()),
        Some(50.0)
    );
}

#[test]
fn colors_functions_and_names() {
    assert_eq!(
        Color::parse("rgb(255, 0, 0)"),
        Some(Color {
            r: 1.,
            g: 0.,
            b: 0.,
            a: 1.
        })
    );
    assert_eq!(Color::parse("rgba(0 0 0 / 50%)").map(|c| c.a), Some(0.5));
    assert_eq!(
        Color::parse("teal").map(|c| (c.g * 255.).round()),
        Some(128.0)
    );
    assert_eq!(Color::parse("transparent").map(|c| c.a), Some(0.0));
    assert_eq!(Color::parse("не-цвет"), None);
}
