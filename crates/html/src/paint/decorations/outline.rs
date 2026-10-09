//! Outline rings outside the border box, without changing layout.

use crate::style::computed::{Computed, OUTLINE_DOUBLE};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, Styled, div, px};

pub(crate) fn decorations(c: &Computed) -> Vec<AnyElement> {
    let mut out = Vec::new();
    // `outline`: рамка ВНЕ коробки и без влияния на раскладку — отдельный
    // абсолютный слой с отрицательным отступом ровно на её толщину.
    // У `border-shape` сплошной контур повторяет фигуру слоем НАД группой
    // (`grouped` → `Grouped::over`, `Computed::shaped_outline`).
    if let Some(o) = c.outline.clone().filter(|_| c.shaped_outline().is_none()) {
        // Шрифтовые единицы ширины и сдвига решаются своим кеглем.
        let em = match c.font_size {
            Some(Len::Px(v)) => v,
            _ => 16.0,
        };
        let px_of = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            Some(Len::Em(k)) => k * em,
            _ => 0.0,
        };
        // Незаданная толщина — `medium` (css-ui-4, начальное значение):
        // длинная запись `outline-style: auto` без толщины давала ноль, и
        // контур молча пропадал (`outline-offset-inset-005` зелёна впустую).
        let w = match o.width {
            None => 3.0,
            other => px_of(other),
        };
        // Рисуется только ЗАДАННЫЙ видимый стиль: начальное `outline-style`
        // — `none`, и `outline: 3px red` без стиля не рисуется вовсе.
        let visible = o.style.is_some_and(|s| s != 0);
        // Цвет без своего: при `auto` — `accent-color` коробки (css-ui-4
        // §outline-color «represents the accent color»; `outline-color-003`:
        // `outline: inherit` берёт стиль `auto`, а цвет — от СВОЕГО
        // `accent-color`), иначе цвет текста, а без него — начальный
        // `CanvasText`, чёрный. Прежде документ без единого `color` оставлял
        // `c.color` пустым и гасил контур целиком (`outline-offset-inset-001`
        // и `-003` зелёны пустотой на обеих сторонах).
        let colour = o
            .color
            .or(if o.style == Some(2) {
                c.accent_color
            } else {
                None
            })
            .or(c.color)
            .or(Some(crate::style::values::value::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            }));
        if let (true, true, Some(colour)) = (visible, w > 0.0, colour) {
            // Отрицательный сдвиг вжимает контур внутрь коробки, но внешняя
            // сторона фигуры не может стать уже удвоенной толщины
            // (css-ui-4 §outline-offset; Blink `outline_painter.cc`
            // `AdjustedOutlineOffset`: `max(offset, -size/2)` по каждой оси
            // отдельно). Зажимается по ЗАДАННОМУ размеру коробки — иного на
            // сборке нет (`outline-013…016`).
            let off = {
                // `outline-offset: inset` — минус толщина (css-ui-4).
                let raw = if o.inset { -w } else { px_of(o.offset) };
                let half = |l: Option<Len>| match l {
                    Some(Len::Px(v)) => Some(v / 2.0),
                    Some(Len::Em(k)) => Some(k * em / 2.0),
                    _ => None,
                };
                match (half(c.width), half(c.height)) {
                    (Some(hw), Some(hh)) => raw.max(-hw.min(hh)),
                    (Some(hw), None) => raw.max(-hw),
                    (None, Some(hh)) => raw.max(-hh),
                    (None, None) => raw,
                }
            };
            // Угол контура повторяет угол коробки, раздвинутый сдвигом и
            // толщиной (css-ui-4 §outline): доля решается так же, как у
            // рамки, — прежде она читалась нулём (`outline-005`).
            let corner = crate::style::apply::radius_px(c, c.radius.tl)
                .filter(|v| *v > 0.0)
                .map_or(0.0, |v| v + off + w);
            // Абсолютный ребёнок отсчитывается от padding-box (CSS 2.1
            // §10.1), а контур лежит снаружи BORDER-box (css-ui-4 §outline:
            // «outside the border edge»): края сдвигаются ещё и на рамку.
            // Прежде при `border: 10px` контур ложился на 10 px внутрь — поверх
            // рамки (border-shape-outline-with-border-ref: красный контур между
            // зелёной рамкой и фоном).
            let bw = c.borders();
            let bpx = |l: Option<Len>| match l {
                Some(Len::Px(v)) => v,
                Some(Len::Em(k)) => k * em,
                _ => 0.0,
            };
            let ring_at = |reach: f32, width: f32, radius: f32| {
                let mut ring = div()
                    .absolute()
                    .top(px(-(reach + bpx(bw.top))))
                    .left(px(-(reach + bpx(bw.left))))
                    .right(px(-(reach + bpx(bw.right))))
                    .bottom(px(-(reach + bpx(bw.bottom))))
                    .border(px(width))
                    .border_color(colour.to_hsla())
                    .rounded(px(radius));
                // Outlines and CSS borders share the device-pixel edge grid
                // (CSS UI 4 outline-style refers to border-style). Keeping
                // a fractional inner outline edge can expose a border that
                // occupies the same geometric edge after border snapping.
                // A translucent outline retains edge coverage like a
                // translucent background. The two double-outline rings
                // retain their shared fractional partition as well.
                ring.style().css_border_snap = Some(o.style == Some(1) && colour.a >= 1.0);
                ring
            };
            let third = (w / 3.0).round();
            if o.style == Some(OUTLINE_DOUBLE) && third >= 1.0 {
                // CSS UI 4 §outline-style, CSS Borders 4 §border-style:
                // both lines and the gap occupy the specified total width.
                // Blink core/paint/border_shape_painter.cc:312-326.
                out.push(ring_at(off + w, third, corner).into_any_element());
                let inner_corner = (corner - w + third).max(0.0);
                out.push(ring_at(off + third, third, inner_corner).into_any_element());
                return out;
            }
            let mut ring = ring_at(off + w, w, corner);
            // Узор контура — тем же примитивом, что узор рамки
            // (`apply::apply_paint`): `dotted`/`dashed` шли сплошной, а эталон
            // `outline-style-012-ref` пишет ту же фигуру `border: 4px dotted`.
            match o.style {
                Some(3) => ring.style().border_style = Some(gpui::BorderStyle::Dotted),
                Some(4) => ring = ring.border_dashed(),
                _ => {}
            }
            out.push(ring.into_any_element());
        }
    }
    out
}
