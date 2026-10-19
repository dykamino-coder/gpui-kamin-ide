//! Скругление углов плиток фона по углам области краски.

use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::{Bounds, Corners, Pixels, px};

/// Скругление — у ОБЛАСТИ краски (css-backgrounds-3 §5.3: фон режется
/// кривой края коробки), а не у каждой плитки: угол плитки скругляется
/// радиусом своего угла коробки, только когда он и есть угол области
/// краски (`origin-border-box_with_radius`: плитка 160 в коробке 514×288
/// скруглялась всеми четырьмя углами).
pub(super) fn corners(
    c: &Computed,
    clip: Bounds<Pixels>,
) -> impl Fn(Bounds<Pixels>) -> Corners<Pixels> {
    let r = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v.max(0.0),
        _ => 0.0,
    };
    let radii = [
        r(c.radius.tl),
        r(c.radius.tr),
        r(c.radius.br),
        r(c.radius.bl),
    ];
    let near = |a: f32, b: f32| (a - b).abs() < 0.5;
    let (pl, pt) = (f32::from(clip.origin.x), f32::from(clip.origin.y));
    let (pr, pb) = (
        pl + f32::from(clip.size.width),
        pt + f32::from(clip.size.height),
    );
    move |cell: Bounds<Pixels>| {
        let (l, t) = (f32::from(cell.origin.x), f32::from(cell.origin.y));
        let (rr, b) = (
            l + f32::from(cell.size.width),
            t + f32::from(cell.size.height),
        );
        gpui::Corners {
            top_left: px(if near(l, pl) && near(t, pt) {
                radii[0]
            } else {
                0.0
            }),
            top_right: px(if near(rr, pr) && near(t, pt) {
                radii[1]
            } else {
                0.0
            }),
            bottom_right: px(if near(rr, pr) && near(b, pb) {
                radii[2]
            } else {
                0.0
            }),
            bottom_left: px(if near(l, pl) && near(b, pb) {
                radii[3]
            } else {
                0.0
            }),
        }
    }
}
