//! Составление полотна многослойной маски на сетке устройства.

use super::mask_layer;
use crate::paint::effects::grouped_element::Grouped;
use gpui::{Bounds, Pixels, Window, px};

#[allow(clippy::too_many_arguments)]
pub(crate) fn composite_mask(
    group: &Grouped,
    layers: &[String],
    bounds: Bounds<Pixels>,
    bw: f32,
    bh: f32,
    ol: f32,
    ot: f32,
    window: &mut Window,
    len: impl Fn(crate::style::values::value::Len, f32, f32) -> f32,
) -> Option<(std::sync::Arc<gpui::RenderImage>, Bounds<Pixels>, u32)> {
    let sf = window.scale_factor();
    let (cw, ch) = (
        (bw * sf).round().max(1.0) as u32,
        (bh * sf).round().max(1.0) as u32,
    );
    // Укладка СВОЕГО слоя (css-masking-1 §7.6-7.7). Список короче
    // набора слоёв повторяется (css-backgrounds-3 §2.2); пустой —
    // старое поведение, одно значение на все слои.
    let repeat_of = |i: usize| -> (bool, bool) {
        let v = &group.mask_repeat_list;
        if v.is_empty() {
            group.mask_no_repeat
        } else {
            v[i % v.len()]
        }
    };
    // Точка укладки слоя: доля — от СВОБОДНОГО места (коробка
    // минус плитка), `right`/`bottom` зеркалят отсчёт — та же
    // арифметика, что на однослойном пути ниже.
    let pos_of = |i: usize, tw: f32, th: f32| -> (f32, f32) {
        let v = &group.mask_pos_list;
        let pick = if v.is_empty() {
            group
                .mask_pos
                .map(|(x, y)| (x, y, group.mask_pos_far.0, group.mask_pos_far.1))
        } else {
            Some(v[i % v.len()])
        };
        let Some((x, y, fx, fy)) = pick else {
            return (0.0, 0.0);
        };
        let one = |l: crate::style::values::value::Len, free: f32, far: bool| {
            let val = match l {
                crate::style::values::value::Len::Pct(p) => p * free,
                l => len(l, free, 0.0),
            };
            if far { free - val } else { val }
        };
        (one(x, bw - tw, fx), one(y, bh - th, fy))
    };
    let built: Vec<crate::paint::background::MaskLayer> = layers
        .iter()
        .enumerate()
        .filter_map(|(i, l)| mask_layer(i, l, group, bw, bh, sf, &pos_of, &repeat_of))
        .collect();
    // Snapped tiles need a canvas on the device grid: its origin
    // rounds, and the tiles keep their exact device positions.
    let (mut built, mut origin, mut size) = (
        built,
        gpui::point(bounds.origin.x + px(ol), bounds.origin.y + px(ot)),
        gpui::size(px(bw), px(bh)),
    );
    if !group.mask_repeat_modes.is_empty() {
        let (ex, ey) = (f32::from(origin.x) * sf, f32::from(origin.y) * sf);
        let (rx, ry) = (ex.round(), ey.round());
        for layer in &mut built {
            layer.tile[0] += ex - rx;
            layer.tile[1] += ey - ry;
        }
        origin = gpui::point(px(rx / sf), px(ry / sf));
        size = gpui::size(px(cw as f32 / sf), px(ch as f32 / sf));
    }
    let img = crate::paint::background::compose_mask_layers(&built, cw, ch)?;
    Some((
        img,
        Bounds { origin, size },
        // Светимость уже учтена при сборке полотна.
        3,
    ))
}
