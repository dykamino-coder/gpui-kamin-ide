//! Запись native OOF по листам и воспроизведение записей.

use super::*;

impl PageStack {
    /// Paint every native source once into per-sheet immutable records.
    pub(in crate::layout::page::page_stack) async fn record_native(
        &self,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> Result<(), PaintRecordError> {
        let natives = std::mem::take(&mut *self.oof.natives.borrow_mut());
        let records = Records::default();
        let mut orphans = HashMap::<usize, Vec<PaintRecord>>::new();
        for native in natives {
            let (sheets, bare) = native.record_async(window, cx, stack).await?;
            for (id, record) in sheets.into_records() {
                let page = usize::try_from(id.0).map_err(|_| PaintRecordError::InvalidGeometry)?;
                orphans.entry(page).or_default().push(record);
            }
            records.bare.absorb(&bare);
        }
        let bare = records.bare.with_natives(self.oof.slot_scopes.natives());
        *self.oof.records.borrow_mut() = Records { orphans, bare };
        Ok(())
    }

    /// Own root records for the legacy copies (`paint::continuation`).
    pub(in crate::layout::page::page_stack) fn copy_records(&self) -> Option<BareRecords> {
        let records = self.oof.records.borrow();
        (!records.bare.is_empty()).then(|| records.bare.clone())
    }

    /// Replay sheet `i`'s source records under its presentation matrix.
    pub(in crate::layout::page::page_stack) fn paint_native(
        &self,
        bounds: Bounds<Pixels>,
        i: usize,
        clip: Bounds<Pixels>,
        window: &mut Window,
    ) -> Result<(), PaintRecordError> {
        let records = self.oof.records.borrow();
        let Some(page) = records.orphans.get(&i) else {
            return Ok(());
        };
        let plan = PaintPlan::sequence(page.iter().cloned().map(PaintPlan::record));
        let origin = point(px(0.), px(0.));
        window
            .with_paint_presentation(self.sheet_matrix(bounds, i), |window| {
                window.paint_plan(
                    &plan,
                    PaintPlacement {
                        source: origin,
                        destination: origin,
                        clip,
                    },
                )
            })?
            .map(|_| ())
    }
}
