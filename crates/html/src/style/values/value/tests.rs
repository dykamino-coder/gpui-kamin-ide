//! Тесты разбора значений: длины с единицами (в том числе корневыми шрифтовыми)
//! и цвета в записях hex/функций/имён по css-values-4 и css-color-4.

use super::*;

#[test]
fn lengths() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    assert_eq!(Len::parse("12px", vcx), Some(Len::Px(12.0)));
    assert_eq!(Len::parse(" 1.5rem ", vcx), Some(Len::Px(24.0)));
    assert_eq!(Len::parse("50%", vcx), Some(Len::Pct(0.5)));
    assert_eq!(Len::parse("auto", vcx), Some(Len::Auto));
    // Голое число принимаем: модель часто пишет `padding: 8`.
    assert_eq!(Len::parse("8", vcx), Some(Len::Px(8.0)));
    assert_eq!(Len::parse("нет", vcx), None);
}

#[test]
fn root_font_units_parse() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    // `rch`/`rex`/`ric`/`rcap` — точки по шрифту корня, а не обрывок `1r`
    // + `ch` (прежде объявление падало целиком).
    for unit in ["rcap", "rch", "rex", "ric"] {
        assert!(
            matches!(Len::parse(&format!("2{unit}"), vcx), Some(Len::Px(v)) if v > 0.0),
            "{unit}"
        );
    }
}

#[test]
fn colors_hex() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    assert_eq!(
        Color::parse("#fff", vcx),
        Some(Color {
            r: 1.,
            g: 1.,
            b: 1.,
            a: 1.
        })
    );
    let c = Color::parse("#8ab4f8", vcx).unwrap();
    assert!((c.r - 0.541).abs() < 0.01 && (c.b - 0.972).abs() < 0.01);
    assert_eq!(
        Color::parse("#00000080", vcx).map(|c| (c.a * 100.).round()),
        Some(50.0)
    );
}

#[test]
fn colors_functions_and_names() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    assert_eq!(
        Color::parse("rgb(255, 0, 0)", vcx),
        Some(Color {
            r: 1.,
            g: 0.,
            b: 0.,
            a: 1.
        })
    );
    assert_eq!(
        Color::parse("rgba(0 0 0 / 50%)", vcx).map(|c| c.a),
        Some(0.5)
    );
    assert_eq!(
        Color::parse("teal", vcx).map(|c| (c.g * 255.).round()),
        Some(128.0)
    );
    assert_eq!(Color::parse("transparent", vcx).map(|c| c.a), Some(0.0));
    assert_eq!(Color::parse("не-цвет", vcx), None);
}
