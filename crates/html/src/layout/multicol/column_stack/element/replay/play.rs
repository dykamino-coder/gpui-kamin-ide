//! Воспроизведение куска из записи главной копии: сегмент куска — на своём месте, под
//! маской, которую уже поставил вызывающий.

use crate::layout::multicol::column_stack::ColumnStack;
use gpui::{
    PaintPlacement, PaintPlan, PaintRecordError, PaintSegmentId, PaintSegmentRecords, Window,
    point, px,
};

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
}
