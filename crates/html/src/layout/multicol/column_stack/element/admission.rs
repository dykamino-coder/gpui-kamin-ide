//! Собственный допуск узлов в записи главной копии: неделимый монолит, переполнивший
//! кусок своего начала, рисуется целиком в этом фрагментаинере и ни в каком другом
//! (css-break-3 §4.1; Blink кладёт монолит в фрагмент его начала, переполнение — его
//! ink overflow, `BoxFragmentPainter` рисует его там). Остальные узлы наследуют срез
//! своего куска. Элемент flex рисуется с куска своего первого фрагмента, как его копия
//! (`paginate::watch::owner`).

use crate::layout::fragment::types::Frag;
use crate::layout::multicol::column_stack::ColumnStack;
use crate::tolerance::COORD_EPS;
use gpui::{Bounds, PaintSegment, PaintSegmentSlice, Pixels, point, px};
use std::collections::HashMap;
use std::rc::Rc;

/// Допуск узлов ребёнка и расширения масок кусков, где монолиты переполняют срез.
#[derive(Default)]
pub(super) struct Nodes {
    pub(super) slices: HashMap<u64, Rc<[PaintSegmentSlice]>>,
    /// Маска воспроизведения куска (по номеру копии) поверх его среза, в координатах окна.
    pub(super) masks: HashMap<usize, Bounds<Pixels>>,
}

impl ColumnStack {
    /// Монолиты ребёнка `kid`, переполнившие кусок своего начала: срез — срез куска,
    /// продолженный вниз на всю протяжённость монолита и вылет `spill`.
    pub(super) fn node_admission(
        &self,
        kid: usize,
        pieces: &[(&Frag, PaintSegment, Bounds<Pixels>)],
        spill: Pixels,
    ) -> Nodes {
        let mut out = Nodes::default();
        let c = &self.children[kid];
        let (Some(r), Some(w)) = (c.regrow.as_ref(), c.watched.as_ref()) else {
            return out;
        };
        let Some(blocks) = w.blocks() else {
            return out;
        };
        for (id, top, bottom) in
            crate::layout::fragment::push::monolith_flow::monoliths(blocks, &r.laid)
        {
            let Some(&(f, placement, clip)) = pieces
                .iter()
                .find(|(f, _, _)| f.from - COORD_EPS <= top && top < f.from + f.h - COORD_EPS)
            else {
                continue;
            };
            let end = f.from + f.h;
            if bottom <= end + COORD_EPS {
                continue;
            }
            let source_clip = Bounds::from_corners(
                clip.origin,
                point(clip.right(), clip.bottom() + px(bottom - end) + spill),
            );
            let delta = placement.destination - placement.source;
            let placed = Bounds::new(source_clip.origin + delta, source_clip.size);
            out.masks
                .entry(f.copy)
                .and_modify(|m| *m = m.union(&placed))
                .or_insert(placed);
            out.slices.insert(
                id,
                Rc::from([PaintSegmentSlice {
                    placement,
                    source_clip,
                }]),
            );
        }
        // Элемент flex рисует кусок своего первого фрагмента (начало margin box) и
        // следующие; в первом его переполнение над срезом видно до маски предка
        // (`paginate::watch::owner`).
        for (id, owner) in w.owners() {
            if out.slices.contains_key(&id) {
                continue;
            }
            let own: Vec<_> = pieces
                .iter()
                .filter(|(f, _, _)| f.copy >= owner)
                .map(|&(f, placement, clip)| PaintSegmentSlice {
                    placement,
                    source_clip: if f.copy == owner {
                        Bounds::from_corners(
                            point(clip.left(), clip.top() - spill),
                            clip.bottom_right(),
                        )
                    } else {
                        clip
                    },
                })
                .collect();
            if let Some(first) = own.first() {
                let delta = first.placement.destination - first.placement.source;
                let placed = Bounds::new(first.source_clip.origin + delta, first.source_clip.size);
                out.masks
                    .entry(owner)
                    .and_modify(|m| *m = m.union(&placed))
                    .or_insert(placed);
            }
            out.slices.insert(id, own.into());
        }
        out
    }
}
