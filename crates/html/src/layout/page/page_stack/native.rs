//! Независимые OOF-продолжения листов: один native paint на источник (группу либо повтор
//! fixed на листе). Копия группы на листе воспроизводит собственные записи корней; лист
//! без копии получает запись источника под матрицей листа (тот же экземпляр эффектов).

use super::PageStack;
use crate::hooks::BareRecords;
use crate::layout::fragment::types::Frag;
use gpui::{
    App, Bounds, PaintPlacement, PaintPlan, PaintRecord, PaintRecordError, PaintSegmentId, Pixels,
    Window, point, px,
};
use std::collections::HashMap;
mod entries;

/// Source records by sheet (sheets without a copy of the ancestors) and the
/// own root records replayed by the sheets' legacy copies.
#[derive(Default)]
pub(super) struct Records {
    orphans: HashMap<usize, Vec<PaintRecord>>,
    pub(super) bare: BareRecords,
}

impl PageStack {
    /// Route every OOF fragment of the plan to its sheet and prepaint each native
    /// source once, its hitboxes projected through the stack matrix.
    pub(super) fn prepaint_native<const QUERY: bool>(
        &self,
        bounds: Bounds<Pixels>,
        plan: &[Frag],
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), PaintRecordError> {
        let mut entries = self.native_entries::<QUERY>(bounds, plan, window)?;
        entries.sort_by_key(|(owner, _, _)| *owner);
        let hits = self.stack_matrix(bounds);
        let mut natives = Vec::with_capacity(entries.len());
        for (_, snapshot, admission) in entries {
            let native = snapshot
                .native
                .clone()
                .ok_or(PaintRecordError::InvalidGeometry)?;
            let routing = admission.finish(&snapshot);
            // Placements replayed by a slot are hit through that copy's scope
            // (its sheet matrix and its own CSS transforms).
            let scopes = crate::hooks::presented_hit_scopes(
                self.oof.slot_scopes.scopes(&routing.roots),
                &routing.presented,
                window,
            );
            window.with_hitbox_transform(hits, |window| {
                crate::layout::geometry::fragments::with_hit_scopes::<QUERY, _>(
                    scopes,
                    window,
                    |window| {
                        native.prepaint(routing.hits.0, routing.hits.1, routing.paint, window, cx)
                    },
                )
            })?;
            natives.push(native);
        }
        self.oof.slot_scopes.clear();
        *self.oof.natives.borrow_mut() = natives;
        Ok(())
    }

    /// Paint every native source once into per-sheet immutable records.
    pub(super) fn record_native(
        &self,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), PaintRecordError> {
        let natives = std::mem::take(&mut *self.oof.natives.borrow_mut());
        let records = Records::default();
        let mut orphans = HashMap::<usize, Vec<PaintRecord>>::new();
        for native in natives {
            let (sheets, bare) = native.record(window, cx)?;
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
    pub(super) fn copy_records(&self) -> Option<BareRecords> {
        let records = self.oof.records.borrow();
        (!records.bare.is_empty()).then(|| records.bare.clone())
    }

    /// Replay sheet `i`'s source records under its presentation matrix.
    pub(super) fn paint_native(
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
