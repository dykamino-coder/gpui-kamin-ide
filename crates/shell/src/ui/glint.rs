//! GlintSurface — фирменная 1px градиентная кромка плавающих панелей
//! (plan/24 §1). CSS-оригинал: 4-стоповый linear-gradient(135deg) в border-box
//! + заливка bg-mantle в padding-box.
//!
//! gpui 0.2.2 даёт максимум 2 стопа на градиент → 4-стоповый glint собран
//! из ДВУХ наложенных 2-стоповых слоёв поверх сплошного mid:
//!   слой A: edge(α .18) @0% → edge(α 0) @22%   (блик верхнего-левого угла)
//!   слой B: edge(α 0) @78% → edge(α .18) @100% (блик нижнего-правого)
//! За пределами стопов градиент клампится к ближайшему стопу — между 22% и
//! 78% оба слоя прозрачны, остаётся чистый mid. Итог идентичен оригиналу.
//! Внутренний rect inset 1px радиусом 15 = padding-box заливка.

use gpui::prelude::*;
use gpui::{AnyElement, Rgba, div, linear_color_stop, linear_gradient, px};
use kamin_metrics as m;
use kamin_theme::Palette;

use crate::colors::rgba;

fn edge(p: &Palette, alpha: f32) -> Rgba {
    let mut c = rgba(p.glint_edge);
    c.a = alpha;
    c
}

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

/// glint_surface с «дырой» под composition-вебвью: те же 4 слоя (mid, две
/// градиентные кромки, mantle inset 1px), но рисуются paint_quad-ами через
/// content-mask сегментов вокруг зоны — градиенты остаются полноразмерными
/// (пиксели идентичны обычной карточке, швов нет), а зона остаётся
/// прозрачной: сквозь неё виден underlay-вебвью.
pub fn glint_surface_wv_holed(p: &Palette, content: AnyElement) -> impl IntoElement {
    let mid = rgba(p.glint_mid);
    let a = p.glint_edge.a;
    let e1 = edge(p, a);
    let e0 = edge(p, 0.0);
    let mantle = rgba(p.bg_mantle);
    div()
        .size_full()
        .relative()
        .child(
            gpui::canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    // Все зоны composition-вью кадра; режем те, что
                    // пересекают эту карточку (свои зоны), остальные не
                    // пересекутся — сегментация вернёт bounds целиком
                    // В режиме CEF дырок НЕТ вовсе: страница живёт в кадре,
                    // а не в окне поверх нас, поэтому вырезать под неё нечего
                    // (`plan/101-cef.md`, Ф5).
                    let holes: Vec<[f32; 4]> = Vec::new();
                    let grad1 = linear_gradient(
                        135.,
                        linear_color_stop(e1, 0.0),
                        linear_color_stop(e0, 0.22),
                    );
                    let grad2 = linear_gradient(
                        135.,
                        linear_color_stop(e0, 0.78),
                        linear_color_stop(e1, 1.0),
                    );
                    let mantle = if false
                    /* KAMIN_VWV_PAINTDBG законстанчен OFF (диагностика plan/97) */
                    {
                        gpui::Rgba {
                            r: 0.0,
                            g: 0.2,
                            b: 1.0,
                            a: 1.0,
                        }
                    } else {
                        mantle
                    };
                    let inner = gpui::Bounds::new(
                        bounds.origin + gpui::point(px(1.), px(1.)),
                        gpui::size(bounds.size.width - px(2.), bounds.size.height - px(2.)),
                    );
                    for seg in hole_segments_multi(bounds, &holes) {
                        window.with_content_mask(Some(gpui::ContentMask { bounds: seg }), |w| {
                            w.paint_quad(gpui::fill(bounds, mid).corner_radii(px(m::RADIUS_LG)));
                            w.paint_quad(gpui::fill(bounds, grad1).corner_radii(px(m::RADIUS_LG)));
                            w.paint_quad(gpui::fill(bounds, grad2).corner_radii(px(m::RADIUS_LG)));
                            w.paint_quad(
                                gpui::fill(inner, mantle).corner_radii(px(m::RADIUS_LG - 1.0)),
                            );
                        });
                    }
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
