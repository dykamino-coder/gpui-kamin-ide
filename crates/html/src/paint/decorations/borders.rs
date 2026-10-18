//! Слои рамок и цветных сторон коробки.

use crate::document::state::ValueCx;
use crate::style::computed::Computed;
use crate::style::values::value::{Len, px_lossy_zero};
use gpui::{AnyElement, IntoElement, Styled, div, px};

#[allow(clippy::too_many_arguments)]
pub(crate) fn box_borders(c: &Computed, out: &mut Vec<AnyElement>, vcx: ValueCx<'_>) {
    if let Some((colour, [t, r, b, l])) = crate::style::apply::border_layer(c, vcx) {
        let mut layer = div()
            .absolute()
            .top(px(-t))
            .left(px(-l))
            .right(px(-r))
            .bottom(px(-b))
            .border_t(px(t))
            .border_r(px(r))
            .border_b(px(b))
            .border_l(px(l))
            .border_color(colour.to_hsla());
        // The layer IS the box's CSS border: it takes the same device-pixel
        // edge snapping as the quad border (`apply::apply_paint`; CSS 2.1
        // §8.5.3, Blink box_border_painter.cc snaps outer and inner rects).
        layer.style().css_border_snap = Some(true);
        if c.border_dashed == Some(true) {
            layer = layer.border_dashed();
        }
        if c.border_dotted == Some(true) {
            layer.style().border_style = Some(gpui::BorderStyle::Dotted);
        }
        // Скругление — как у квада (`apply::apply_radius`): при маске группы
        // или `border-shape` квад углов не получает, и слой тоже.
        if !c.radius_masked(vcx) && c.border_shape.is_none() {
            let rad = |l: Option<Len>| crate::style::apply::radius_px(c, l, vcx).unwrap_or(0.0);
            layer = layer
                .rounded_tl(px(rad(c.radius.tl)))
                .rounded_tr(px(rad(c.radius.tr)))
                .rounded_br(px(rad(c.radius.br)))
                .rounded_bl(px(rad(c.radius.bl)));
        }
        out.push(layer.into_any_element());
    }

    // Рамка `double` (css-backgrounds-3 §4.2) — двумя кольцами: линии по
    // трети толщины, зазор — остаток (Blink
    // `BorderEdge::GetDoubleBorderStripeWidths`: 50 → 17/16/17, 5 → 2/1/2).
    // Квад и слой рамки её не красят (`apply::double_border`).
    if let Some((colour, [t, r, b, l])) = crate::style::apply::double_border(c, vcx) {
        let third = |w: f32| (w / 3.0).round();
        let (o_t, o_r, o_b, o_l) = (third(t), third(r), third(b), third(l));
        let mut outer = div()
            .absolute()
            .top(px(-t))
            .left(px(-l))
            .right(px(-r))
            .bottom(px(-b))
            .border_t(px(o_t))
            .border_r(px(o_r))
            .border_b(px(o_b))
            .border_l(px(o_l))
            .border_color(colour.to_hsla());
        // Внутренняя линия прилегает к padding-box снаружи.
        let mut inner = div()
            .absolute()
            .top(px(-o_t))
            .left(px(-o_l))
            .right(px(-o_r))
            .bottom(px(-o_b))
            .border_t(px(o_t))
            .border_r(px(o_r))
            .border_b(px(o_b))
            .border_l(px(o_l))
            .border_color(colour.to_hsla());
        if !c.radius_masked(vcx) {
            let rad = |x: Option<Len>| crate::style::apply::radius_px(c, x, vcx).unwrap_or(0.0);
            let shrink = |x: Option<Len>, a: f32, b: f32| (rad(x) - a.max(b)).max(0.0);
            outer = outer
                .rounded_tl(px(rad(c.radius.tl)))
                .rounded_tr(px(rad(c.radius.tr)))
                .rounded_br(px(rad(c.radius.br)))
                .rounded_bl(px(rad(c.radius.bl)));
            inner = inner
                .rounded_tl(px(shrink(c.radius.tl, t - o_t, l - o_l)))
                .rounded_tr(px(shrink(c.radius.tr, t - o_t, r - o_r)))
                .rounded_br(px(shrink(c.radius.br, b - o_b, r - o_r)))
                .rounded_bl(px(shrink(c.radius.bl, b - o_b, l - o_l)));
        }
        out.push(outer.into_any_element());
        out.push(inner.into_any_element());
    }

    // Рамка при фигурных углах (`corner-shape`, css-borders-4): внешний край
    // — контур, внутренний — он же, сжатый на толщину сторон. Квад её не
    // красит (`apply::apply_paint`); слой — цветной растр кольца в
    // border-box, тем же растеризатором, что и маска группы, — контуры
    // совпадают попиксельно. Разные цвета сторон остаются полосами ниже.
    {
        if let Some((spec, colour, widths)) = super::shaped_ring::spec(c, vcx) {
            let [t, r, b, l] = widths;
            out.push(
                gpui::canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        super::shaped_ring::paint(&spec, colour, bounds, window);
                    },
                )
                // Абсолютный ребёнок считается от padding-box — кольцо
                // накрывает рамку отрицательными отступами (как полосы ниже).
                .absolute()
                .top(px(-t))
                .left(px(-l))
                .right(px(-r))
                .bottom(px(-b))
                .into_any_element(),
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn coloured_borders(c: &Computed, out: &mut Vec<AnyElement>, vcx: ValueCx<'_>) {
    let sides: Vec<_> = c.border_colors.iter().flatten().collect();
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (30.09): не класть полосу, когда все ЗАДАННЫЕ
    // стороны одного цвета (квад и так красит их, `apply::apply_paint`
    // `!mixed`), — ради удвоенной альфы односторонней полупрозрачной рамки
    // (`grid-gap-decorations-067…082`). Свод v210: +7/−31 — 24 пары
    // `border-image-*` (полосы легли поверх картинки; с прежним правилом для
    // картинки срез щелей/фонов/line-clamp 1329 пар дал +7/−7). Сами 067…082
    // почти не сдвинулись (075/076 0.51 → 0.48), зато непрозрачные
    // односторонние `border-bottom` эталонов `019/048-051` ушли 0.11 → 0.61,
    // пунктирные `016` 0.35 → 1.01: квад рисует одностороннюю рамку не
    // точка в точку как полоса. Выигрыш `007`, `fragmentation-020/021/
    // 025/026` — от того же снятия полосы; разбирать по снимкам обеих сторон.
    let uniform = sides.len() == 4 && sides.iter().all(|s| *s == sides[0]);
    // При `border-shape` рамка — один слой цветом relevant side (спека:
    // stroke-from-border), прямоугольные полосы сторон ей не нужны.
    // A one-colour `double` border is painted whole by its rings.
    let rings = crate::style::apply::double_border(c, vcx).is_some();
    if !sides.is_empty() && !uniform && !rings && c.border_shape.is_none() {
        let side_px = px_lossy_zero;
        let bw = c.borders();
        let (t, r, b, l) = (
            side_px(bw.top),
            side_px(bw.right),
            side_px(bw.bottom),
            side_px(bw.left),
        );
        // Сторона без своего цвета красится общим `border-color`, а без него —
        // цветом текста (`currentColor`): квад при полосах цвета не получает
        // (`apply::apply_paint`, `strips`), и такая сторона иначе пропала бы.
        let fallback = c
            .border_color
            .or(c.color)
            .unwrap_or(crate::style::values::value::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            });
        for (i, colour) in c.border_colors.iter().enumerate() {
            let colour = colour.as_ref().unwrap_or(&fallback);
            let w = [t, r, b, l][i];
            if w <= 0.0 {
                continue;
            }
            // Абсолютный ребёнок считается от ВНУТРЕННЕГО края рамки,
            // поэтому кольцо накрывается отрицательными отступами ровно на
            // толщину сторон — иначе полоса ложится внутрь содержимого.
            let strip = div().absolute();
            let mut strip = match i {
                0 => strip.top(px(-t)).left(px(-l)).right(px(-r)).h(px(w)),
                1 => strip.top(px(-t)).right(px(-r)).bottom(px(-b)).w(px(w)),
                2 => strip.bottom(px(-b)).left(px(-l)).right(px(-r)).h(px(w)),
                _ => strip.top(px(-t)).left(px(-l)).bottom(px(-b)).w(px(w)),
            };
            // A dotted/dashed side stays patterned (css-backgrounds-3 §4.2).
            strip = match c.border_side_styles[i] {
                Some(s @ (7 | 8)) => {
                    strip = match i {
                        0 => strip.border_t(px(w)),
                        1 => strip.border_r(px(w)),
                        2 => strip.border_b(px(w)),
                        _ => strip.border_l(px(w)),
                    }
                    .border_color(colour.to_hsla());
                    let pattern = [gpui::BorderStyle::Dotted, gpui::BorderStyle::Dashed];
                    strip.style().border_style = Some(pattern[usize::from(s == 8)]);
                    strip
                }
                _ => strip.bg(colour.to_hsla()),
            };
            out.push(strip.into_any_element());
        }
    }
}
