//! Resolve page margin box geometry before constructing its painted element.

use crate::flow::*;

/// Раскладка марджин-боксов одного листа (css-page-3 §margin-dimension; Blink
/// `PageContainerLayoutAlgorithm::LayoutAllMarginBoxes`): прямоугольник border
/// box каждой коробки в точках листа и её элемент. Мера — по содержимому
/// (`probe`): главная ось стороны верха/низа — min/max-content ширина, боковых
/// — высота при уже решённой ширине (Blink `EdgeMarginNodePreferredSize`).
pub(crate) fn layout_margin_boxes(
    boxes: Vec<MarginBox>,
    g: &PageGeom,
    window: &mut Window,
    cx: &mut App,
) -> Vec<((f32, f32, f32, f32), AnyElement)> {
    use crate::page_margin::{self as pm, Place, Pref, Side};
    let mut out: Vec<(usize, (f32, f32, f32, f32), AnyElement)> = Vec::new();
    let mut edges: Vec<(Side, [Option<MarginBox>; 3])> = vec![
        (Side::Top, [None, None, None]),
        (Side::Right, [None, None, None]),
        (Side::Bottom, [None, None, None]),
        (Side::Left, [None, None, None]),
    ];
    for mut b in boxes {
        match b.place {
            Place::Corner { top, left } => {
                let cb = pm::containing_block(b.place, g.size, g.margin);
                let available_w = cb.2 - b.margin[1].unwrap_or(0.0) - b.margin[3].unwrap_or(0.0);
                let available_h = cb.3 - b.margin[0].unwrap_or(0.0) - b.margin[2].unwrap_or(0.0);
                b.w = b.w.or_else(|| {
                    margin_box_size::intrinsic(&mut b, true, None, available_w, window, cx)
                });
                let width = b.w.unwrap_or(available_w).max(0.0);
                b.h = b.h.or_else(|| {
                    margin_box_size::intrinsic(&mut b, false, Some(width), available_h, window, cx)
                });
                // Обе оси «растягиваются» по своему полю листа, затем поля
                // решаются у обоих краёв бумаги.
                let w =
                    b.w.unwrap_or(cb.2 - b.margin[1].unwrap_or(0.0) - b.margin[3].unwrap_or(0.0))
                        .max(0.0);
                let h =
                    b.h.unwrap_or(cb.3 - b.margin[0].unwrap_or(0.0) - b.margin[2].unwrap_or(0.0))
                        .max(0.0);
                let (mt, _) = pm::edge_margins(b.margin[0], b.margin[2], h, cb.3, top);
                let (ml, _) = pm::edge_margins(b.margin[3], b.margin[1], w, cb.2, left);
                let key = match (top, left) {
                    (true, true) => 0,
                    (true, false) => 4,
                    (false, false) => 8,
                    (false, true) => 12,
                };
                out.push((key, (cb.0 + ml, cb.1 + mt, w, h), (b.make)(w, h)));
            }
            Place::Edge { side, at } => {
                if let Some(e) = edges.iter_mut().find(|e| e.0 == side) {
                    e.1[at] = Some(b);
                }
            }
        }
    }
    for (side, mut trio) in edges {
        if trio.iter().all(|b| b.is_none()) {
            continue;
        }
        let place = Place::Edge { side, at: 0 };
        let cb = pm::containing_block(place, g.size, g.margin);
        let horiz = side.horizontal();
        let (main_avail, cross_avail) = if horiz { (cb.2, cb.3) } else { (cb.3, cb.2) };
        // Поперечный размер: задан либо во всё поле без полей.
        let cross = |b: &MarginBox| -> f32 {
            let (spec, ma, mb) = if horiz {
                (b.h, b.margin[0], b.margin[2])
            } else {
                (b.w, b.margin[3], b.margin[1])
            };
            spec.unwrap_or(cross_avail - ma.unwrap_or(0.0) - mb.unwrap_or(0.0))
                .max(0.0)
        };
        let mut prefs: [Option<Pref>; 3] = [None; 3];
        for i in 0..3 {
            let Some(b) = trio[i].as_mut() else { continue };
            let (spec, ma, mb) = if horiz {
                (b.w, b.margin[3], b.margin[1])
            } else {
                (b.h, b.margin[0], b.margin[2])
            };
            let margins = ma.unwrap_or(0.0) + mb.unwrap_or(0.0);
            let other = if horiz { None } else { Some(cross(b)) };
            let spec = spec.or_else(|| {
                margin_box_size::intrinsic(b, horiz, other, main_avail - margins, window, cx)
            });
            let (min, max) = match spec {
                Some(v) => (v, v),
                None if horiz => {
                    let lo = b.probe.layout_as_root(
                        size(
                            gpui::AvailableSpace::MinContent,
                            gpui::AvailableSpace::MaxContent,
                        ),
                        window,
                        cx,
                    );
                    let hi = b.probe.layout_as_root(
                        size(
                            gpui::AvailableSpace::MaxContent,
                            gpui::AvailableSpace::MaxContent,
                        ),
                        window,
                        cx,
                    );
                    (f32::from(lo.width), f32::from(hi.width))
                }
                None => {
                    let w = cross(b);
                    let s = b.probe.layout_as_root(
                        size(
                            gpui::AvailableSpace::Definite(px(w)),
                            gpui::AvailableSpace::MaxContent,
                        ),
                        window,
                        cx,
                    );
                    (f32::from(s.height), f32::from(s.height))
                }
            };
            prefs[i] = Some(Pref {
                min,
                max,
                margins,
                auto: spec.is_none(),
            });
        }
        let mains = pm::edge_sizes(prefs, main_avail);
        for i in 0..3 {
            let Some(mut b) = trio[i].take() else {
                continue;
            };
            let main = mains[i];
            let cross_margins = if horiz {
                b.margin[0].unwrap_or(0.0) + b.margin[2].unwrap_or(0.0)
            } else {
                b.margin[1].unwrap_or(0.0) + b.margin[3].unwrap_or(0.0)
            };
            let c = margin_box_size::intrinsic(
                &mut b,
                !horiz,
                Some(main),
                cross_avail - cross_margins,
                window,
                cx,
            )
            .unwrap_or_else(|| cross(&b));
            let (ms, me) = if horiz {
                (b.margin[3].unwrap_or(0.0), b.margin[1].unwrap_or(0.0))
            } else {
                (b.margin[0].unwrap_or(0.0), b.margin[2].unwrap_or(0.0))
            };
            let at_start = matches!(side, Side::Top | Side::Left);
            let (cs, _) = if horiz {
                pm::edge_margins(b.margin[0], b.margin[2], c, cross_avail, at_start)
            } else {
                pm::edge_margins(b.margin[3], b.margin[1], c, cross_avail, at_start)
            };
            let used = main + ms + me;
            let shift = match i {
                0 => 0.0,
                1 => (main_avail - used) / 2.0,
                _ => main_avail - used,
            } + ms;
            let rect = if horiz {
                (cb.0 + shift, cb.1 + cs, main, c)
            } else {
                (cb.0 + cs, cb.1 + shift, c, main)
            };
            // Порядок краски Blink (`LayoutAllMarginBoxes`): по часовой от
            // левого верхнего угла, низ и левая сторона — с конца.
            let key = match side {
                Side::Top => 1 + i,
                Side::Right => 5 + i,
                Side::Bottom => 11 - i,
                Side::Left => 15 - i,
            };
            out.push((key, rect, (b.make)(rect.2, rect.3)));
        }
    }
    out.sort_by_key(|x| x.0);
    out.into_iter().map(|(_, r, e)| (r, e)).collect()
}
