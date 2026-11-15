//! Paint-only style damage (`Computed::same_layout`).

use super::*;
use crate::style::values::value::{Color, Len};

#[test]
fn colour_and_background_colour_are_paint_only() {
    let a = Computed::default();
    let mut b = a.clone();
    b.inh.ui.color = Some(Color {
        r: 0.1,
        g: 0.2,
        b: 0.3,
        a: 1.0,
    });
    b.reset_background.background = Some(Color {
        r: 0.4,
        g: 0.5,
        b: 0.6,
        a: 1.0,
    });
    assert!(a != b && a.same_layout(&b) && b.same_layout(&a));
    let mut c = b.clone();
    c.reset_box_layout.margin.top = Some(Len::Px(1.0));
    assert!(!b.same_layout(&c), "margin is layout");
    let mut d = b.clone();
    d.inh.ui.hidden = Some(true);
    assert!(!b.same_layout(&d), "visibility stays exact");
}

#[test]
fn border_colours_are_paint_only_where_asked() {
    let a = Computed::default();
    let mut b = a.clone();
    b.reset_border.border_colors[1] = Color::BLACK.into();
    b.reset_cascade.side_order.border[2][1] = Default::default();
    assert!(!a.same_layout(&b), "layout damage by default");
    assert!(a.same_layout_but_border_colors(&b));
    let mut c = b.clone();
    c.reset_border.border_width.top = Some(Len::Px(2.0));
    assert!(!b.same_layout_but_border_colors(&c), "width is layout");
}
