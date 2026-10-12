//! GlintSurface — плавающая карточка панели: заливка bg-mantle с 1px-рамкой
//! (plan/24 §1), и сегментация прямоугольника вокруг «дыр».
//!
//! CSS-оригинал рисовал кромку 4-стоповым linear-gradient(135deg) в
//! border-box. Здесь кромка однотонная: градиентный блик стоил три слоя на
//! всю площадь карточки ради одного пикселя у края (INC-2026-0001).

use gpui::prelude::*;
use gpui::{AnyElement, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

use crate::colors::rgba;

/// Сегменты `bounds` вокруг дыры (зона composition-вебвью, логические px):
/// верхняя/нижняя полосы + боковые куски. Дыры нет — весь bounds.
/// Сегменты вокруг НЕСКОЛЬКИХ дыр: последовательное вычитание.
pub fn hole_segments_multi(
    bounds: gpui::Bounds<gpui::Pixels>,
    holes: &[[f32; 4]],
) -> Vec<gpui::Bounds<gpui::Pixels>> {
    let mut segs = vec![bounds];
    for h in holes {
        segs = segs
            .into_iter()
            .flat_map(|b| hole_segments(b, Some(*h)))
            .collect();
    }
    segs
}

pub fn hole_segments(
    bounds: gpui::Bounds<gpui::Pixels>,
    hole: Option<[f32; 4]>,
) -> Vec<gpui::Bounds<gpui::Pixels>> {
    let (bx, by) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
    let (bw, bh) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    let Some([hx, hy, hw, hh]) = hole else {
        return vec![bounds];
    };
    // Пересечение дыры с bounds
    let x0 = hx.max(bx);
    let y0 = hy.max(by);
    let x1 = (hx + hw).min(bx + bw);
    let y1 = (hy + hh).min(by + bh);
    if x1 <= x0 || y1 <= y0 {
        return vec![bounds];
    }
    let seg = |x: f32, y: f32, w: f32, h: f32| {
        gpui::Bounds::new(
            gpui::point(gpui::px(x), gpui::px(y)),
            gpui::size(gpui::px(w), gpui::px(h)),
        )
    };
    let mut out = Vec::with_capacity(4);
    if y0 > by {
        out.push(seg(bx, by, bw, y0 - by)); // верх
    }
    if y1 < by + bh {
        out.push(seg(bx, y1, bw, by + bh - y1)); // низ
    }
    if x0 > bx {
        out.push(seg(bx, y0, x0 - bx, y1 - y0)); // лево
    }
    if x1 < bx + bw {
        out.push(seg(x1, y0, bx + bw - x1, y1 - y0)); // право
    }
    out
}

/// Плавающая карточка: заливка панели с однотонной 1px-рамкой и контент.
///
/// Раньше кромка собиралась из четырёх слоёв на всю площадь карточки (mid,
/// два градиентных блика, заливка поверх), хотя видимым оставался только
/// пиксель у края. Без видеокарты такой кадр растеризует WARP на CPU, и
/// три лишних слоя на каждой карточке стоили около 40 % кадра
/// (INC-2026-0001). Один квад с рамкой рисуется одним проходом.
pub fn glint_surface_wv_holed(p: &Palette, content: AnyElement) -> impl IntoElement {
    let mid = rgba(p.glint_mid);
    let mantle = rgba(p.bg_mantle);
    div()
        .size_full()
        .relative()
        .child(
            gpui::canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    window.paint_quad(
                        gpui::fill(bounds, mantle)
                            .corner_radii(px(m::RADIUS_LG))
                            .border_widths(px(1.))
                            .border_color(mid),
                    );
                },
            )
            .absolute()
            .size_full(),
        )
        .child(
            div()
                .size_full()
                .rounded(px(m::RADIUS_LG))
                .p(px(1.))
                .child(content),
        )
}
