//! Словарь shadow-токенов (variables.css --shadow-* 1:1).
//! Light-тема — свои значения (light-theme.css:139-147): «ink-tinted»
//! rgba(27,26,22) и мягче; у lg/card меняется и геометрия.

use gpui::{BoxShadow, point, px};

/// (y, blur, alpha) тёмной и светлой темы → BoxShadow активной.
fn shadow(dark: (f32, f32, f32), light: (f32, f32, f32)) -> Vec<BoxShadow> {
    let is_light = kamin_theme::current_is_light();
    let ((y, blur, alpha), (r, g, b)) = if is_light {
        (light, (27.0 / 255.0, 26.0 / 255.0, 22.0 / 255.0))
    } else {
        (dark, (0.0, 0.0, 0.0))
    };
    vec![BoxShadow {
        color: gpui::Rgba { r, g, b, a: alpha }.into(),
        offset: point(px(0.), px(y)),
        blur_radius: px(blur),
        spread_radius: px(0.),
    }]
}

/// --shadow-lg: dark 0 8 16 .3 / light 0 6 14 .10
pub fn lg() -> Vec<BoxShadow> {
    shadow((8.0, 16.0, 0.3), (6.0, 14.0, 0.10))
}
/// --shadow-card: dark 0 0 6 .2 / light 0 0 4 .08
pub fn card() -> Vec<BoxShadow> {
    shadow((0.0, 6.0, 0.2), (0.0, 4.0, 0.08))
}
/// --shadow-modal: dark 0 8 32 .5 / light 0 8 32 .18
pub fn modal() -> Vec<BoxShadow> {
    shadow((8.0, 32.0, 0.5), (8.0, 32.0, 0.18))
}
/// --shadow-tab: dark 0 6 18 .45 / light 0 6 18 .14
pub fn tab() -> Vec<BoxShadow> {
    shadow((6.0, 18.0, 0.45), (6.0, 18.0, 0.14))
}
/// --shadow-toast: dark 0 10 40 .4 / light 0 10 40 .16
pub fn toast() -> Vec<BoxShadow> {
    shadow((10.0, 40.0, 0.4), (10.0, 40.0, 0.16))
}
/// --shadow-dropdown: dark 0 4 16 .5 / light 0 4 16 .16
pub fn dropdown() -> Vec<BoxShadow> {
    shadow((4.0, 16.0, 0.5), (4.0, 16.0, 0.16))
}
/// --shadow-mini: dark 0 2 8 .3 / light 0 2 8 .10
pub fn mini() -> Vec<BoxShadow> {
    shadow((2.0, 8.0, 0.3), (2.0, 8.0, 0.10))
}
/// --shadow-bar: dark 0 -4 12 .4 / light 0 -4 12 .10
pub fn bar() -> Vec<BoxShadow> {
    shadow((-4.0, 12.0, 0.4), (-4.0, 12.0, 0.10))
}
/// --shadow-card-popup: dark 0 8 24 .5 / light 0 8 24 .16
pub fn card_popup() -> Vec<BoxShadow> {
    shadow((8.0, 24.0, 0.5), (8.0, 24.0, 0.16))
}
