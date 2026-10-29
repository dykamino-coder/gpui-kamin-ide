//! Допуск элементов flex и сетки в записи главной копии — так, как их рисуют копии
//! (`paginate::watch::owner`, `hook::Watch`): элемент виден с куска своего первого фрагмента
//! (начало margin box), там его переполнение над срезом видно до маски предка; окно копии
//! сдвигает элемент сетки и обрезает его снизу (`ItemWindow`); израсходованная часть
//! элемента flex срезана сверху (`axis::consumed_mask`).

use super::Nodes;
use crate::layout::fragment::flow::blocks::Laid;
use crate::layout::fragment::paginate::watch::Watched;
use crate::layout::fragment::types::Frag;
use gpui::{Bounds, PaintSegment, PaintSegmentSlice, Pixels, point, px};
use std::collections::{HashMap, HashSet};

/// Срезы элементов flex и сетки по кускам записи.
pub(super) fn admission(
    pieces: &[(&Frag, PaintSegment, Bounds<Pixels>)],
    (w, laid): (&Watched, &Laid),
    spill: Pixels,
    out: &mut Nodes,
) {
    let owners = w.owners();
    let (windows, consumed) = w.piece_windows();
    let ids: HashSet<u64> = owners
        .keys()
        .copied()
        .chain(windows.keys().map(|&(_, id)| id))
        .chain(consumed.keys().copied())
        .collect();
    let mut ordinals: HashMap<u64, u32> = HashMap::new();
    for id in ids {
        if out.slices.contains_key(&id) {
            continue;
        }
        let owner = owners.get(&id).copied().unwrap_or(0);
        let top = laid.at(id).map(|(t, _)| t);
        let mut own = Vec::new();
        for &(f, placement, clip) in pieces {
            if f.copy < owner {
                continue;
            }
            let (mut placement, mut clip) = (placement, clip);
            if f.copy == owner && owners.contains_key(&id) {
                clip = Bounds::from_corners(
                    point(clip.left(), clip.top() - spill),
                    clip.bottom_right(),
                );
            }
            if let (Some(&(used, complete)), Some(top)) = (consumed.get(&id), top) {
                if complete {
                    continue;
                }
                let start = placement.source.y + px(top + used);
                clip = Bounds::from_corners(
                    point(clip.left(), clip.top().max(start)),
                    clip.bottom_right(),
                );
            }
            if let Some(win) = windows.get(&(f.copy, id)) {
                // Обрезка окна — от несдвинутого верха элемента на месте копии.
                if let (Some(cut), Some(top)) = (win.clip, top) {
                    let bottom = placement.source.y + px(top + cut);
                    clip = Bounds::from_corners(
                        clip.origin,
                        point(clip.right(), clip.bottom().min(bottom)),
                    );
                }
                if win.shift != 0.0 {
                    let ordinal = ordinals.entry(placement.id.0).or_insert(0);
                    *ordinal += 1;
                    placement.ordinal = *ordinal;
                    placement.destination.y += px(win.shift);
                    clip.origin.y -= px(win.shift);
                    out.routes.push(placement);
                }
            }
            if clip.size.height <= px(0.) {
                continue;
            }
            if f.copy == owner {
                let delta = placement.destination - placement.source;
                let placed = Bounds::new(clip.origin + delta, clip.size);
                out.masks
                    .entry(owner)
                    .and_modify(|m| *m = m.union(&placed))
                    .or_insert(placed);
            }
            own.push(PaintSegmentSlice {
                placement,
                source_clip: clip,
            });
        }
        out.slices.insert(id, own.into());
    }
}
