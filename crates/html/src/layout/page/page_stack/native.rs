//! Независимые OOF-продолжения листов: один native paint на источник (группу либо повтор
//! fixed на листе). Копия группы на листе воспроизводит собственные записи корней; лист
//! без копии получает запись источника под матрицей листа (тот же экземпляр эффектов).

use super::PageStack;
use crate::hooks::BareRecords;
use crate::layout::fragment::axis_slice::{self as geometry, snapped};
use crate::layout::fragment::token::resume::oof::FixedPolicy;
use crate::layout::fragment::types::Frag;
use crate::layout::positioned::containing_block::metrics::Snapshot;
use crate::layout::positioned::containing_block::metrics::fragmentation::paint::Admission;
use gpui::{
    App, Bounds, PaintPlacement, PaintPlan, PaintRecord, PaintRecordError, PaintSegmentId, Pixels,
    Window, point, px,
};
use std::{collections::HashMap, rc::Rc};

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
    pub(super) fn prepaint_native(
        &self,
        bounds: Bounds<Pixels>,
        plan: &[Frag],
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), PaintRecordError> {
        let mut entries = self.native_entries(bounds, plan, window)?;
        entries.sort_by_key(|(owner, _, _)| *owner);
        let hits = self.stack_matrix(bounds);
        let mut natives = Vec::with_capacity(entries.len());
        for (_, snapshot, admission) in entries {
            let native = snapshot
                .native
                .clone()
                .ok_or(PaintRecordError::InvalidGeometry)?;
            let routing = admission.finish(&snapshot);
            window.with_hitbox_transform(hits, |window| {
                native.prepaint(routing.hits.0, routing.hits.1, routing.paint, window, cx)
            })?;
            natives.push(native);
        }
        *self.oof.natives.borrow_mut() = natives;
        Ok(())
    }

    fn native_entries(
        &self,
        bounds: Bounds<Pixels>,
        normal: &[Frag],
        window: &Window,
    ) -> Result<Vec<(usize, Rc<Snapshot>, Admission)>, PaintRecordError> {
        let plan = self.oof.plan.borrow();
        if plan.failure.is_some() {
            return Err(PaintRecordError::InvalidGeometry);
        }
        let sources = self.oof.sources.borrow();
        let snapshots = self.oof.snapshots.borrow();
        let repeats = self.oof.repeat_snapshots.borrow();
        let viewport = window.viewport_size();
        let spill = f32::from(viewport.width.max(viewport.height).max(bounds.size.width));
        let scale = window.scale_factor();
        let mut entries: Vec<(usize, Rc<Snapshot>, Admission)> = Vec::new();
        for f in plan.iter() {
            let (owner, source) = sources
                .get(f.kid)
                .ok_or(PaintRecordError::InvalidGeometry)?;
            let snapshot = if source.policy == FixedPolicy::RepeatPages {
                repeats
                    .iter()
                    .find(|(page, o, _)| *page == f.col && o == owner)
                    .map(|(_, _, s)| s)
            } else {
                snapshots.iter().find(|(o, _)| o == owner).map(|(_, s)| s)
            }
            .ok_or(PaintRecordError::InvalidGeometry)?;
            let flow = snapshot
                .flows
                .iter()
                .find(|s| s.id == source.id)
                .ok_or(PaintRecordError::InvalidGeometry)?;
            let rect = flow.metrics.normal_rect();
            let inline = geometry::inline(self.axis, rect);
            let from = geometry::slice(self.axis, rect, 0., inline, f.from, f.h).origin;
            let offset = if self.axis.is_vertical() {
                rect.origin.y
            } else {
                rect.origin.x
            };
            let page = self.page_area(bounds, f.col);
            let to = geometry::slice(self.axis, page, offset.into(), inline, f.y, f.h).origin;
            let to = from + snapped(to - from, scale);
            // Unbroken edges keep their ink overflow; a break edge cuts it.
            let siblings = || plan.iter().filter(|g| g.kid == f.kid);
            let first = !siblings().any(|g| (g.col, g.from) < (f.col, f.from));
            let last = !siblings().any(|g| (g.col, g.from) > (f.col, f.from));
            let (head, tail) = flow.kid.clone_dec.unwrap_or_default();
            let up = if first { spill } else { -head };
            let down = if last { spill } else { -tail };
            let clip = geometry::slice(
                self.axis,
                rect,
                -spill,
                inline + spill * 2.,
                f.from - up,
                (f.h + up + down).max(0.),
            );
            // The group's copy on this sheet carries the root's own slot.
            let copied = normal.iter().any(|g| g.kid == *owner && g.col == f.col);
            let index = match entries.iter().position(|e| Rc::ptr_eq(&e.1, snapshot)) {
                Some(index) => index,
                None => {
                    entries.push((*owner, snapshot.clone(), Admission::new()));
                    entries.len() - 1
                }
            };
            entries[index].2.insert(
                flow.id,
                PaintSegmentId(f.col as u64),
                from,
                to,
                clip,
                copied,
            )?;
        }
        Ok(entries)
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
        *self.oof.records.borrow_mut() = Records { orphans, ..records };
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
