//! Тесты разбора градиентов в Computed.

use super::*;
use crate::document::state::ValueCx;
use crate::style::css::parse_decls;

fn c(css: &str, vcx: ValueCx<'_>) -> Computed {
    let mut c = Computed::default();
    c.apply_decls(&parse_decls(css), vcx);
    c
}

#[test]
fn radial_gradient_is_recognised_with_its_shape() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let g = c(
        "background: radial-gradient(circle at center, #fff, #000)",
        vcx,
    )
    .gradient
    .unwrap();
    assert!(g.radial && g.circle, "форма окружности обязана дойти");
    let e = c("background: radial-gradient(#fff, #000)", vcx)
        .gradient
        .unwrap();
    assert!(e.radial && !e.circle, "без ключевого слова — эллипс");
}

#[test]
fn every_stop_survives_with_its_position() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let g = c(
        "background: linear-gradient(180deg, #e03131 0%, #fcc419 50%, #2f9e44 100%)",
        vcx,
    )
    .gradient
    .unwrap();
    assert_eq!(
        g.stops.len(),
        3,
        "средний стоп терялся — градиент был двух-цветным"
    );
    assert_eq!(g.stops[1].1, 0.5);
}

#[test]
fn stops_without_positions_spread_evenly() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let g = c("background: linear-gradient(90deg, #000, #888, #fff)", vcx)
        .gradient
        .unwrap();
    assert_eq!(g.stops[1].1, 0.5, "равномерная раскладка, как в CSS");
}
