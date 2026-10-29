//! Воспроизведение куска из записи главной копии: сегмент куска — на своём месте, под
//! маской, которую уже поставил вызывающий; prepaint главной копии по тем же кускам.

use super::Routes;
use crate::layout::fragment::replay_pieces::{LATE, ReplayPieces};
use crate::layout::multicol::column_stack::ColumnStack;
use gpui::{
    App, PaintPlacement, PaintPlan, PaintRecordError, PaintSegmentId, PaintSegmentRecords, Window,
    point, px,
};
use std::rc::Rc;

impl ColumnStack {
    /// Кусок `copy` из записи главной копии.
    pub(super) fn paint_replayed(
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

    /// Продолжения внепоточных корней из записей главных копий — после потока всех
    /// кусков стопки, каждое под областью слотов своего куска.
    pub(in super::super) fn paint_late(
        replayed: &std::collections::HashMap<usize, Option<Option<super::Replayed>>>,
        window: &mut Window,
    ) {
        let mut kids: Vec<_> = replayed.iter().collect();
        kids.sort_by_key(|(kid, _)| **kid);
        for (records, masks) in kids.into_iter().filter_map(|(_, r)| r.as_ref()?.as_ref()) {
            let mut late: Vec<_> = (masks.iter())
                .filter(|(segment, _)| **segment as u64 & LATE != 0)
                .collect();
            late.sort_by_key(|(segment, _)| **segment);
            for (&segment, &bounds) in late {
                window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
                    let _ = Self::paint_replayed(records, segment, window);
                });
            }
        }
    }

    /// Prepaint главной копии: её hitboxes стоят в каждом куске под его срезом.
    pub(in super::super) fn prepaint_replayed(
        routes: &Routes,
        window: &mut Window,
        cx: &mut App,
        prepaint: impl FnOnce(&mut Window, &mut App),
    ) -> Result<(), PaintRecordError> {
        let slices = routes.slices.clone();
        let slots = Rc::new(ReplayPieces {
            records: None,
            pieces: routes.slots.clone(),
        });
        routes.admit(true);
        let done = crate::hooks::with_replay_pieces(slots, window, |window| {
            window.with_prepaint_segments(routes.extent, routes.routes.clone(), cx, |window, cx| {
                window.with_paint_segment_slices(slices, |window| prepaint(window, cx))
            })
        });
        routes.admit(false);
        done?
    }
}
