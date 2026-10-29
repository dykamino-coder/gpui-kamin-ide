//! Сегменты `LATE` записи главной копии: продолжения внепоточных корней
//! (`continuation::replayed`) и отложенные задания кадра записи
//! (`coordinator::with_late_jobs`) — то, что копия куска рисовала после потока всех кусков.

use crate::layout::fragment::fragment_mask;
use crate::layout::fragment::replay_pieces::{LATE, ReplayPiece};
use crate::layout::fragment::types::Frag;
use crate::layout::multicol::column_stack::ColumnStack;
use gpui::{Bounds, PaintSegment, PaintSegmentId, PaintSegmentSlice, Pixels};
use std::rc::Rc;

impl ColumnStack {
    /// Кусок `f` для слотов: свой сегмент `LATE` с размещением куска; слоты копии рисуют
    /// продолжения под полосой фрагментаинера и срезом куска (`with_copy_area`).
    pub(in super::super) fn slot_piece(
        &self,
        f: &Frag,
        placement: PaintSegment,
        (clip, mask): (Bounds<Pixels>, Bounds<Pixels>),
        (bounds, spill, scale): (Bounds<Pixels>, Pixels, f32),
    ) -> ReplayPiece {
        let area = (self.fragmentainer_band(f.col, bounds, spill))
            .map_or(mask, |b| fragment_mask::snap(b.union(&clip), scale).bounds);
        ReplayPiece {
            placement: PaintSegment {
                id: PaintSegmentId(LATE | f.copy as u64),
                ..placement
            },
            column: PaintSegmentId(f.col as u64),
            area,
        }
    }

    /// Сегменты слотов входят в запись; их области (в координатах источника) — в её охват.
    pub(in super::super) fn slot_routes(
        slots: &[ReplayPiece],
        routes: &mut Vec<PaintSegment>,
        extent: Option<Bounds<Pixels>>,
    ) -> Option<Bounds<Pixels>> {
        slots.iter().fold(extent, |extent, s| {
            routes.push(s.placement);
            let delta = s.placement.destination - s.placement.source;
            let area = Bounds::new(s.area.origin - delta, s.area.size);
            extent.map(|e| e.union(&area))
        })
    }
}

impl super::Routes {
    /// Срезы сегментов `LATE` для отложенных заданий: размещение куска и его срез потока.
    pub(super) fn late_slices(&self) -> Rc<[PaintSegmentSlice]> {
        (self.slots.iter().zip(self.slices.iter()))
            .map(|(s, slice)| PaintSegmentSlice {
                placement: s.placement,
                source_clip: slice.source_clip,
            })
            .collect()
    }

    /// Маски воспроизведения сегментов `LATE`: маска куска (под ней копия рисовала
    /// отложенные задания), со слотами — и их область.
    pub(super) fn late_masks(
        &self,
        natives: bool,
        masks: &mut std::collections::HashMap<usize, Bounds<Pixels>>,
    ) {
        for (s, slice) in self.slots.iter().zip(self.slices.iter()) {
            let delta = s.placement.destination - s.placement.source;
            let piece = Bounds::new(slice.source_clip.origin + delta, slice.source_clip.size);
            let area = if natives { s.area.union(&piece) } else { piece };
            let m = masks.entry(s.placement.id.0 as usize).or_insert(area);
            *m = m.union(&area);
        }
    }
}
