//! Формы подписей таблицы вокруг сетки рядов.

use super::super::TableBands;
use crate::dom::Element;
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::clone::solid_box;
use crate::layout::fragment::fragment_size::shape_full;

#[allow(clippy::too_many_arguments)]
pub(crate) fn caption_slots_shape(
    depth: u8,
    cx: ShapeCx,
    bands: &mut TableBands,
    mt: f32,
    mb: f32,
    caps_top: Vec<&Element>,
    caps_bot: Vec<&Element>,
    mut cuts: Vec<(f32, f32)>,
    mut forced: Vec<f32>,
    mut solid: Vec<(f32, f32)>,
    h_box: f32,
) -> Option<(f32, f32, f32, Vec<(f32, f32)>, Vec<f32>, Vec<(f32, f32)>)> {
    let mut cap_slots: Vec<Option<&Element>> =
        Vec::with_capacity(caps_top.len() + caps_bot.len() + 1);
    cap_slots.extend(caps_top.iter().copied().map(Some));
    cap_slots.push(None);
    cap_slots.extend(caps_bot.iter().copied().map(Some));
    let mut wy = 0.0f32;
    let mut wcuts: Vec<(f32, f32)> = Vec::new();
    let mut wforced: Vec<f32> = Vec::new();
    let mut wsolid: Vec<(f32, f32)> = Vec::new();
    let mut prev_mb = 0.0f32;
    let mut through = 0.0f32;
    let mut last_mb = 0.0f32;
    let mut wfirst = true;
    let mut wforce_next = false;
    for slot in cap_slots {
        let (ih, imt, imb, icuts, iforced, isolid, ifb, ifa) = match slot {
            None => (
                h_box,
                0.0,
                0.0,
                std::mem::take(&mut cuts),
                std::mem::take(&mut forced),
                std::mem::take(&mut solid),
                false,
                false,
            ),
            Some(cap) => {
                let (ch, cmt, cmb, ccuts, cforced, csolid) = shape_full(cap, depth - 1, cx)?;
                // Монолитная подпись (`contain: size`, `break-inside: avoid`,
                // прокрутка, замещаемая) — сплошной диапазон во всю высоту,
                // как у любого ребёнка блочной стопки (`shape_full`, ветка
                // `solid_box`).
                let csolid = if solid_box(cap) {
                    vec![(0.0, ch)]
                } else {
                    csolid
                };
                (
                    ch,
                    cmt,
                    cmb,
                    ccuts,
                    cforced,
                    csolid,
                    cap.style.break_before_force,
                    cap.style.break_after_force,
                )
            }
        };
        // У первого поле уходит СКВОЗЬ верх обёртки: своих рамки и отбивки у
        // неё нет (CSS 2.1 §8.3.1).
        let lead = if wfirst {
            through = imt;
            0.0
        } else {
            prev_mb.max(imt)
        };
        if !wfirst {
            wcuts.push((wy, wy + lead));
            if ifb || wforce_next {
                wforced.push(wy);
            }
        }
        wforce_next = ifa;
        let start = wy + lead;
        // Полосы секций — в координаты обёртки: коробка рядов стоит под
        // верхними подписями.
        if slot.is_none() {
            for s in [&mut bands.head, &mut bands.foot].into_iter().flatten() {
                s.0 += start;
            }
            bands.box_top += start;
            bands.box_end += start;
        }
        wcuts.extend(
            icuts
                .into_iter()
                .map(|(need, nf)| (start + need, start + nf)),
        );
        wforced.extend(iforced.into_iter().map(|f| start + f));
        wsolid.extend(isolid.into_iter().map(|(a, b)| (start + a, start + b)));
        wy = start + ih;
        prev_mb = imb;
        last_mb = imb;
        wfirst = false;
    }
    let h = wy;
    wcuts.retain(|&(need, _)| need > 0.01 && need < h - 0.01);
    wforced.retain(|&f| f > 0.01 && f < h - 0.01);
    Some((h, mt.max(through), mb.max(last_mb), wcuts, wforced, wsolid))
}
