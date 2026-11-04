//! Воспроизведение куска из записи главной копии: сегмент куска — на своём месте, под
//! маской, которую уже поставил вызывающий; сегменты `LATE` после потока всех кусков;
//! prepaint главной копии по тем же кускам.

use super::{Replayed, Routes};
use crate::layout::fragment::replay_pieces::{LATE, ReplayPieces};
use gpui::{
    App, Bounds, PaintPlacement, PaintPlan, PaintRecordError, PaintSegmentId, PaintSegmentRecords,
    Pixels, Window, point, px,
};
use std::collections::HashMap;
use std::rc::Rc;

/// Кусок `copy` из записи главной копии.
pub(crate) fn paint_segment(
    records: &PaintSegmentRecords,
    copy: usize,
    window: &mut Window,
) -> Result<(), PaintRecordError> {
    let Some(record) = records.get(PaintSegmentId(copy as u64)) else {
        return Ok(());
    };
    window.paint_plan(
        &PaintPlan::record(record.clone()),
        PaintPlacement {
            source: point(px(0.), px(0.)),
            destination: point(px(0.), px(0.)),
            clip: window.content_mask().bounds,
        },
    )?;
    Ok(())
}

/// Сегменты `LATE` записи с их масками воспроизведения, по порядку кусков.
pub(crate) fn late_segments(
    masks: &HashMap<usize, Bounds<Pixels>>,
) -> Vec<(usize, Bounds<Pixels>)> {
    let mut late: Vec<_> = (masks.iter())
        .filter(|(segment, _)| **segment as u64 & LATE != 0)
        .map(|(&segment, &bounds)| (segment, bounds))
        .collect();
    late.sort_by_key(|(segment, _)| *segment);
    late
}

/// Продолжения внепоточных корней и отложенные задания из записей главных копий — после
/// потока всех кусков стопки, каждое под областью своего куска (маски — в координатах окна).
pub(crate) fn paint_late(replayed: &HashMap<usize, Option<Option<Replayed>>>, window: &mut Window) {
    let mut kids: Vec<_> = replayed.iter().collect();
    kids.sort_by_key(|(kid, _)| **kid);
    for (records, masks) in kids.into_iter().filter_map(|(_, r)| r.as_ref()?.as_ref()) {
        for (segment, bounds) in late_segments(masks) {
            window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
                let _ = paint_segment(records, segment, window);
            });
        }
    }
}

impl Routes {
    /// Hitboxes главной копии проходят общий стек продолжений под маршрутами кусков.
    pub(crate) async fn prepaint_async(
        &self,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
        prepaint: impl AsyncFnOnce(&mut Window, &mut App, &mut gpui::PaintStack),
    ) -> Result<(), PaintRecordError> {
        let slices = self.slices.clone();
        let slots = Rc::new(ReplayPieces {
            records: None,
            pieces: self.slots.clone(),
        });
        self.admit(true);
        let _admission = super::Admission(self);
        crate::hooks::with_replay_pieces_async(slots, window, async |window| {
            window
                .with_prepaint_segments_async(
                    self.extent,
                    self.routes.clone(),
                    cx,
                    stack,
                    async |window, cx, stack| {
                        window
                            .with_paint_segment_slices_async(slices, async |window| {
                                prepaint(window, cx, stack).await
                            })
                            .await
                    },
                )
                .await
        })
        .await?
    }
}
