//! Градиент из пяти и более стопов полосами.
// owner: A

use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px};

pub(crate) fn gradient_stripes(
    c: &Computed,
    out: &mut Vec<gpui::AnyElement>,
) {
    // Градиент из пяти и более стопов: заливка несёт четыре (патч GPUI), а
    // дальше осевой градиент по-прежнему рисуется полосами — по слою на пару
    // соседних стопов. Наклонный полосами не выразить.
    // Градиент, ушедший в растровую плитку (`gradient_as_tile`), полосами не
    // дублируется: они красили ВСЮ коробку поверх плитки — мимо размера,
    // повтора и пространства смешения.
    if let Some(g) = c.gradient.as_ref().filter(|_| !c.gradient_as_tile()) {
        let vertical = matches!(g.angle_deg as i32, 0 | 180);
        let horizontal = matches!(g.angle_deg as i32, 90 | 270);
        let reverse = matches!(g.angle_deg as i32, 0 | 270);
        // Стопы в точках рисуются полосами точной ширины: доля от них не
        // считается, длина оси известна только коробке. Отсчёт полос — от
        // верха/лева; обратное направление (0/270deg) идёт от низа/права.
        if !g.stops_px.is_empty() && !g.radial && (vertical || horizontal) {
            // Полосы в точках могут выйти за коробку (стопы длиннее оси) —
            // фон обрезается её краем, поэтому все полосы живут в общем
            // обрезающем слое на всю коробку.
            let mut bands: Vec<AnyElement> = vec![];
            // Фиксация css-images-3 §3.5.3 п.2: позиция не меньше наибольшей
            // из предыдущих. Без неё пара `green 4em, red 3em` (обратный
            // порядок — так пишут жёсткий край) пропускалась целиком, и под
            // полосами оставался долевой градиент на всю коробку
            // (`white-space-intrinsic-size-017/018`).
            let mut fixed = g.stops_px.clone();
            for i in 1..fixed.len() {
                fixed[i].1 = fixed[i].1.max(fixed[i - 1].1);
            }
            // До первого стопа — его цвет, после последнего — цвет последнего
            // (§3.5.3): полосы между стопами этого места не красили.
            const FAR: f32 = 1.0e5;
            let mut ext = Vec::with_capacity(fixed.len() + 2);
            if let Some(first) = fixed.first() {
                ext.push((first.0, first.1.min(0.0) - FAR));
            }
            ext.extend(fixed.iter().copied());
            if let Some(last) = fixed.last() {
                ext.push((last.0, last.1 + FAR));
            }
            for pair in ext.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                let (p0, p1) = (a.1, b.1);
                if p1 <= p0 {
                    continue;
                }
                let (from, to) = (a.0, b.0);
                let band = crate::style::computed::Gradient {
                    angle_deg: if vertical { 180.0 } else { 90.0 },
                    radial: false,
                    circle: false,
                    from: if reverse { to } else { from },
                    to: if reverse { from } else { to },
                    stops: vec![(from, 0.0), (to, 1.0)],
                    stops_px: vec![],
                    stops_raw: vec![],
                    // Полоса наследует пространство интерполяции исходного градиента.
                    space: g.space,
                    hue: g.hue,
                };
                let layer = div().absolute().bg(crate::style::apply::fill(&band));
                bands.push(
                    match (vertical, reverse) {
                        (true, false) => layer.left_0().right_0().top(px(p0)).h(px(p1 - p0)),
                        (true, true) => layer.left_0().right_0().bottom(px(p0)).h(px(p1 - p0)),
                        (false, false) => layer.top_0().bottom_0().left(px(p0)).w(px(p1 - p0)),
                        (false, true) => layer.top_0().bottom_0().right(px(p0)).w(px(p1 - p0)),
                    }
                    .into_any_element(),
                );
            }
            out.push(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .overflow_hidden()
                    .children(bands)
                    .into_any_element(),
            );
        } else if g.stops.len() > 4 && !g.radial && (vertical || horizontal) {
            // Полоса перекрывает фон родителя целиком, поэтому скругление
            // приходится повторять на крайних полосах: иначе углы блока
            // становятся прямыми.
            let corner = |l: Option<Len>| match l {
                Some(Len::Px(v)) => v,
                _ => 0.0,
            };
            let last_band = g.stops.len() - 2;
            for (idx, pair) in g.stops.windows(2).enumerate() {
                let (a, b) = (pair[0], pair[1]);
                let (mut p0, mut p1) = (a.1, b.1);
                if p1 <= p0 {
                    continue;
                }
                let (mut from, mut to) = (a.0, b.0);
                if reverse {
                    // Отсчёт полос всегда сверху/слева, поэтому обратное
                    // направление разворачивает и порядок, и цвета.
                    (p0, p1) = (1.0 - p1, 1.0 - p0);
                    (from, to) = (to, from);
                }
                let band = crate::style::computed::Gradient {
                    angle_deg: g.angle_deg,
                    radial: false,
                    circle: false,
                    from,
                    to,
                    stops: vec![(from, 0.0), (to, 1.0)],
                    stops_px: vec![],
                    stops_raw: vec![],
                    // Полоса наследует пространство интерполяции исходного градиента.
                    space: g.space,
                    hue: g.hue,
                };
                let mut layer = div().absolute().bg(crate::style::apply::fill(&band));
                // «Первая» полоса по направлению отрисовки, а не по списку:
                // при обратном направлении список развёрнут.
                let first_edge = if reverse { idx == last_band } else { idx == 0 };
                let last_edge = if reverse { idx == 0 } else { idx == last_band };
                if vertical {
                    if first_edge {
                        layer = layer
                            .rounded_tl(px(corner(c.radius.tl)))
                            .rounded_tr(px(corner(c.radius.tr)));
                    }
                    if last_edge {
                        layer = layer
                            .rounded_bl(px(corner(c.radius.bl)))
                            .rounded_br(px(corner(c.radius.br)));
                    }
                } else {
                    if first_edge {
                        layer = layer
                            .rounded_tl(px(corner(c.radius.tl)))
                            .rounded_bl(px(corner(c.radius.bl)));
                    }
                    if last_edge {
                        layer = layer
                            .rounded_tr(px(corner(c.radius.tr)))
                            .rounded_br(px(corner(c.radius.br)));
                    }
                }
                out.push(
                    if vertical {
                        layer
                            .left_0()
                            .right_0()
                            .top(gpui::relative(p0))
                            .h(gpui::relative(p1 - p0))
                    } else {
                        layer
                            .top_0()
                            .bottom_0()
                            .left(gpui::relative(p0))
                            .w(gpui::relative(p1 - p0))
                    }
                    .into_any_element(),
                );
            }
        }
    }
}
