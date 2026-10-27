//! Один source prepaint и paint обслуживают normal и независимые OOF-размещения.

use super::*;
use crate::layout::positioned::containing_block::metrics::fragmentation::paint::Admission;
use gpui::{
    PaintPlacement, PaintPlan, PaintRecordError, PaintSegment, PaintSegmentId, PaintSegmentSlice,
};
use std::{collections::HashMap, rc::Rc};
mod clips;
mod geometry;
mod tables;

impl ColumnStack {
    pub(crate) fn native_kid(&self, _kid: usize) -> bool {
        self.oof.geometry.native.is_some()
    }

    pub(crate) fn prepaint_native(
        &mut self,
        bounds: Bounds<Pixels>,
        plan: &[Frag],
        spans: &[(usize, f32)],
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), PaintRecordError> {
        let Some(native) = self.oof.geometry.native.clone() else {
            return Ok(());
        };
        let snapshot = self.oof.geometry.clone();
        let mut routes = Vec::new();
        let mut ordinals = HashMap::<usize, u32>::new();
        let mut admission = Admission::new();
        let spill = window
            .viewport_size()
            .width
            .max(window.viewport_size().height)
            .max(bounds.size.width);
        let mut append = |family: u64,
                          col: usize,
                          source: gpui::Point<Pixels>,
                          destination: gpui::Point<Pixels>,
                          clip: Bounds<Pixels>|
         -> Result<(), PaintRecordError> {
            let ordinal = ordinals.entry(col).or_default();
            let placement = PaintSegment {
                id: PaintSegmentId(col as u64),
                ordinal: *ordinal,
                source,
                destination,
            };
            *ordinal = ordinal
                .checked_add(1)
                .ok_or(PaintRecordError::InvalidGeometry)?;
            routes.push(placement);
            admission.insert(
                family,
                PaintSegmentSlice {
                    placement,
                    source_clip: clip,
                },
            )
        };
        for f in plan {
            if !self.native_kid(f.kid) {
                continue;
            }
            let kid = &self.children[f.kid];
            let id = kid
                .watched
                .as_ref()
                .and_then(|w| w.blocks())
                .map(|b| b.id)
                .ok_or(PaintRecordError::InvalidGeometry)?;
            let rect = *snapshot
                .normal
                .get(&id)
                .ok_or(PaintRecordError::InvalidGeometry)?;
            let (column, row) = self.place(f.col);
            let source = geometry::slice(
                self.axis,
                rect,
                0.,
                geometry::inline(self.axis, rect),
                f.from,
                f.h,
            )
            .origin;
            let destination = geometry::slice(
                self.axis,
                bounds,
                column as f32 * (self.col_w.get() + self.gap) + kid.par.dx,
                self.col_w.get(),
                row + f.y,
                f.h,
            )
            .origin
                + point(px(kid.rel.0), px(kid.rel.1));
            let inline_spill = if self.axis.is_vertical() {
                window.viewport_size().height.max(bounds.size.height)
            } else {
                window.viewport_size().width.max(bounds.size.width)
            };
            let clip = self.native_clip(rect, f, plan, inline_spill.into());
            append(id, f.col, source, destination, clip)?;
        }
        for &(index, y) in spans {
            if !self.native_kid(index) {
                continue;
            }
            let kid = &self.children[index];
            let id = kid
                .watched
                .as_ref()
                .and_then(|w| w.blocks())
                .map(|b| b.id)
                .ok_or(PaintRecordError::InvalidGeometry)?;
            let rect = *snapshot
                .normal
                .get(&id)
                .ok_or(PaintRecordError::InvalidGeometry)?;
            append(
                id,
                plan.iter().map(|f| f.col).max().unwrap_or(0) + index + 1,
                rect.origin,
                geometry::slice(
                    self.axis,
                    bounds,
                    0.,
                    geometry::inline(self.axis, bounds),
                    y,
                    geometry::block(self.axis, rect),
                )
                .origin,
                Bounds::new(
                    rect.origin - point(spill, spill),
                    rect.size + size(spill + spill, spill + spill),
                ),
            )?;
        }
        let oof = self.oof.plan.borrow();
        if oof.failure.is_some() {
            return Err(PaintRecordError::InvalidGeometry);
        }
        let sources = self.oof.sources.borrow();
        for f in &oof.fragments {
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
            let source = geometry::slice(
                self.axis,
                rect,
                0.,
                geometry::inline(self.axis, rect),
                f.from,
                f.h,
            )
            .origin;
            let inline = if self.axis.is_vertical() {
                rect.top() - flow.owner_rect.top()
            } else {
                rect.left() - flow.owner_rect.left()
            };
            let destination = geometry::slice(
                self.axis,
                actual.rect,
                inline.into(),
                geometry::inline(self.axis, rect),
                f.y,
                f.h,
            )
            .origin;
            let (head, tail) = flow.kid.clone_dec.unwrap_or_default();
            let clip = geometry::slice(
                self.axis,
                rect,
                -f32::from(spill),
                geometry::inline(self.axis, rect) + f32::from(spill) * 2.,
                f.from + head,
                (f.h - head - tail).max(0.),
            );
            append(id, f.col, source, destination, clip)?;
        }
        drop(sources);
        drop(oof);
        let mut slices = admission.nodes(&snapshot);
        slices.slots = self.table_slots(bounds, plan, &mut routes, &mut ordinals, window, cx)?;
        native.prepaint(Rc::from(routes), slices, window, cx)
    }

    pub(crate) fn paint_native(
        &self,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), PaintRecordError> {
        let Some(native) = &self.oof.geometry.native else {
            return Ok(());
        };
        let records = native.record(window, cx)?;
        let plan = PaintPlan::sequence(
            records
                .into_records()
                .into_iter()
                .map(|(_, record)| PaintPlan::record(record)),
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
