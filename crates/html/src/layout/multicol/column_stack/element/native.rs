//! Независимые OOF-продолжения колонок: один source prepaint и один native paint. Копия
//! предков в колонке воспроизводит собственную запись корня; колонка без копии получает
//! допуск записи источника с её единственным экземпляром эффектов предков.

use super::*;
use crate::layout::fragment::axis_slice::{self as geometry, snapped};
use crate::layout::positioned::containing_block::metrics::fragmentation::paint::{
    Admission, Placement,
};
use gpui::{PaintPlacement, PaintPlan, PaintRecordError, PaintSegmentId};

impl ColumnStack {
    /// Legacy copies of a column carry the slots of its continued roots.
    pub(crate) fn copy_records(&self) -> Option<crate::hooks::BareRecords> {
        self.oof
            .geometry
            .native_along(self.axis.is_vertical())
            .map(|_| self.oof.records.borrow().1.clone())
    }

    pub(crate) fn prepaint_native(
        &mut self,
        bounds: Bounds<Pixels>,
        plan: &[Frag],
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), PaintRecordError> {
        let Some(native) = self.oof.geometry.native_along(self.axis.is_vertical()).cloned() else {
            return Ok(());
        };
        let snapshot = self.oof.geometry.clone();
        let oof = self.oof.plan.borrow();
        if oof.failure.is_some() {
            return Err(PaintRecordError::InvalidGeometry);
        }
        let sources = self.oof.sources.borrow();
        let viewport = window.viewport_size();
        let spill = f32::from(viewport.width.max(viewport.height).max(bounds.size.width));
        let scale = window.scale_factor();
        let mut admission = Admission::new();
        for f in oof.iter() {
            eprintln!(
                "P08DBG plan kid {} col {} y {} from {} h {} (src {:?})",
                f.kid,
                f.col,
                f.y,
                f.from,
                f.h,
                sources.get(f.kid)
            );
            let id = *sources
                .get(f.kid)
                .ok_or(PaintRecordError::InvalidGeometry)?;
            let flow = snapshot
                .flows
                .iter()
                .find(|s| s.id == id)
                .ok_or(PaintRecordError::InvalidGeometry)?;
            let actual = self
                .fragmentainer(f.col, bounds)
                .map_err(|_| PaintRecordError::InvalidGeometry)?;
            let rect = flow.metrics.normal_rect();
            // The source paints the box where its layout put it; the normal
            // rect above holds the resolved static position the plan used.
            let laid = snapshot.normal.get(&flow.id).copied().unwrap_or(rect);
            let inline_size = geometry::inline(self.axis, rect);
            let source = geometry::slice(self.axis, laid, 0., inline_size, f.from, f.h).origin;
            // The source lays the columns' content out from its origin: the inline
            // offset there includes every ancestor's offset (owner margins too).
            let inline = if self.axis.is_vertical() {
                rect.top()
            } else {
                rect.left()
            };
            let destination =
                geometry::slice(self.axis, actual.rect, inline.into(), inline_size, f.y, f.h)
                    .origin;
            let destination = source + snapped(destination - source, scale);
            // Unbroken edges keep their ink overflow; a break edge cuts it.
            let siblings = || oof.iter().filter(|g| g.kid == f.kid);
            let first = !siblings().any(|g| (g.col, g.from) < (f.col, f.from));
            let last = !siblings().any(|g| (g.col, g.from) > (f.col, f.from));
            let (head, tail) = flow.kid.clone_dec.unwrap_or_default();
            let up = if first { spill } else { -head };
            let down = if last { spill } else { -tail };
            let clip = geometry::slice(
                self.axis,
                laid,
                -spill,
                inline_size + spill * 2.,
                f.from - up,
                (f.h + up + down).max(0.),
            );
            // The box's own decoration spans the whole fragment: a broken box
            // fills the rest of its fragmentainer (css-break-3 §5.4).
            let extent = if last {
                f.h
            } else {
                (geometry::block(self.axis, actual.rect) - f.y).max(f.h)
            };
            let decoration = geometry::slice(
                self.axis,
                laid,
                -spill,
                inline_size + spill * 2.,
                f.from - if first { spill } else { 0. },
                extent + if first { spill } else { 0. } + if last { spill } else { 0. },
            );
            // The plan placed normal geometry; relative positioning of the box and its
            // ancestors is a paint displacement on top (CSS 2.1 §9.4.3).
            let shift = snapshot.relative.get(&flow.id).copied().unwrap_or_default();
            let copied = self.owner_copy(flow.owner, f.col, plan);
            admission.insert(Placement {
                family: id,
                root: flow.root,
                segment: PaintSegmentId(f.col as u64),
                source,
                destination: destination + shift,
                clip,
                decoration,
                copied,
            })?;
        }
        drop(sources);
        drop(oof);
        let routing = admission.finish(&snapshot);
        // Placements replayed by a slot are hit through that copy's scope.
        let scopes = self.oof.slot_scopes.scopes(&routing.roots);
        self.oof.slot_scopes.clear();
        window.with_prepaint_segment_hit_scopes(scopes, |window| {
            native.prepaint(routing.hits.0, routing.hits.1, routing.paint, window, cx)
        })
    }

    /// A normal kid owning `owner` has a legacy copy in `col`.
    fn owner_copy(&self, owner: u64, col: usize, plan: &[Frag]) -> bool {
        self.children
            .iter()
            .position(|c| c.source == Some(owner))
            .is_some_and(|kid| plan.iter().any(|f| f.kid == kid && f.col == col))
    }

    /// Paint the source once; its own records wait for the column copies.
    pub(crate) fn record_native(
        &self,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), PaintRecordError> {
        let Some(native) = self.oof.geometry.native_along(self.axis.is_vertical()) else {
            return Ok(());
        };
        let (records, bare) = native.record(window, cx)?;
        *self.oof.records.borrow_mut() = (records.into_records(), bare);
        Ok(())
    }

    /// Columns without a copy of the ancestors: the source record's single
    /// instance of their effects, after the column's normal flow.
    pub(crate) fn paint_orphans(&self, window: &mut Window) -> Result<(), PaintRecordError> {
        let records = self.oof.records.borrow();
        if records.0.is_empty() {
            return Ok(());
        }
        let plan = PaintPlan::sequence(
            records
                .0
                .iter()
                .map(|(_, record)| PaintPlan::record(record.clone())),
        );
        window.paint_plan(
            &plan,
            PaintPlacement {
                source: point(px(0.), px(0.)),
                destination: point(px(0.), px(0.)),
                clip: window.content_mask().bounds,
            },
        )?;
        Ok(())
    }
}
