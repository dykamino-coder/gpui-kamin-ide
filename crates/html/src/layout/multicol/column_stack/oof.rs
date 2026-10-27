//! Адаптер фактических колонок: данные владельца, без выбора высоты или баланса.

use super::ColumnStack;
use crate::layout::fragment::plan::oof::stream::{Input, Streams};
use crate::layout::fragment::plan::{Completion, Plan};
use crate::layout::fragment::token::{Error, Space};
use crate::layout::fragment::types::axis_box;
use crate::layout::positioned::containing_block::metrics::Snapshot;
use crate::style::computed::writing_mode::WritingMode;
use gpui::{Bounds, Pixels};
use std::{cell::RefCell, rc::Rc};
mod balance;
mod source;

#[derive(Default)]
pub(crate) struct State {
    pub(crate) geometry: Rc<Snapshot>,
    pub(crate) wm: WritingMode,
    pub(crate) plan: RefCell<Plan>,
    pub(crate) sources: RefCell<Vec<u64>>,
    pub(crate) pending: RefCell<Option<Streams>>,
    /// Source records without a copy of the ancestors, and own root records.
    pub(crate) records: RefCell<(
        Vec<(gpui::PaintSegmentId, gpui::PaintRecord)>,
        crate::hooks::BareRecords,
    )>,
    pub(crate) owners: Vec<(u64, usize, f32)>,
    /// Hit scopes of the slots in this stack's copies, read by the native prepaint.
    pub(crate) slot_scopes: crate::hooks::SlotScopes,
}

impl State {
    pub(crate) fn fresh(&self) -> Self {
        Self {
            geometry: self.geometry.clone(),
            wm: self.wm,
            owners: self.owners.clone(),
            ..Default::default()
        }
    }
}

/// Прямоугольник и Space имеют одно происхождение: завершённый normal plan.
pub(crate) struct Fragmentainer {
    pub(crate) space: Space,
    pub(crate) rect: Bounds<Pixels>,
}

impl ColumnStack {
    pub(crate) fn oof_for_measurement(&self) -> State {
        let mut state = self.oof.fresh();
        state.owners = self
            .children
            .iter()
            .enumerate()
            .filter_map(|(index, c)| Some((c.source?, index, c.h)))
            .collect();
        state
    }

    pub(crate) fn with_oof(mut self, geometry: Option<Rc<Snapshot>>, wm: WritingMode) -> Self {
        self.oof = State {
            geometry: geometry.unwrap_or_default(),
            wm,
            ..Default::default()
        };
        self
    }

    pub(crate) fn fragmentainer(
        &self,
        col: usize,
        bounds: Bounds<Pixels>,
    ) -> Result<Fragmentainer, Error> {
        let wrap = self.rows.is_some_and(|r| r.wrap);
        let lines = self.lines_plan.borrow();
        let &(at, h) = lines
            .get(if wrap { col / self.count } else { 0 })
            .ok_or(Error::InvalidGeometry)?;
        if !h.is_finite() || h <= 0. {
            return Err(Error::InvalidGeometry);
        }
        let w = self.col_w.get();
        let (_, block) = self.axis_sizes(bounds);
        Ok(Fragmentainer {
            space: Space {
                wm: self.oof.wm,
                inline_size: Some(w),
                block_size: Some(h),
                ..Space::columns()
            },
            rect: axis_box(
                self.axis,
                bounds.origin,
                block,
                (if wrap { col % self.count } else { col }) as f32 * (w + self.gap),
                w,
                at,
                h,
            ),
        })
    }

    pub(crate) fn plan_oof(&self, normal: &Plan, bounds: Bounds<Pixels>) {
        let mut plan = Plan::default();
        if let Err(error) = self.resume_oof(normal, bounds, &mut plan) {
            plan.failure = Some(error);
            plan.incomplete();
        }
        if normal.completion != Completion::Complete {
            plan.incomplete();
        }
        *self.oof.plan.borrow_mut() = plan;
    }

    fn resume_oof(
        &self,
        normal: &Plan,
        bounds: Bounds<Pixels>,
        result: &mut Plan,
    ) -> Result<(), Error> {
        let (ids, inputs) = self.oof_inputs(normal)?;
        *self.oof.sources.borrow_mut() = ids;
        self.oof.pending.borrow_mut().take();
        if inputs.is_empty() {
            return Ok(());
        }
        let mut streams = Streams::new(inputs)?;
        let count = normal.iter().map(|f| f.col + 1).max().unwrap_or(1);
        let mut index = 0;
        while index < count || streams.needs_space() {
            // Ряды выдаёт владелец ColumnStack. Незавершённый OOF сохраняется,
            // пока владелец не предоставит следующий реальный ряд (P11).
            if self.rows.is_some_and(|r| r.wrap)
                && index / self.count >= self.lines_plan.borrow().len()
            {
                *self.oof.pending.borrow_mut() = Some(streams);
                result.incomplete();
                return Ok(());
            }
            let actual = self.fragmentainer(index, bounds)?;
            debug_assert_eq!(
                actual.space.block_size,
                Some(f32::from(if self.axis.is_vertical() {
                    actual.rect.size.width
                } else {
                    actual.rect.size.height
                }))
            );
            let batch = streams.resume(
                index,
                actual.space,
                index + 1 >= count && normal.completion == Completion::Complete,
            )?;
            result
                .fragments
                .extend(batch.fragments.into_iter().map(|(id, mut f)| {
                    f.kid = id as usize;
                    f
                }));
            index = index.checked_add(1).ok_or(Error::ResourceExhausted)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
