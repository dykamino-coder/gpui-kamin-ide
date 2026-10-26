//! Пробы баланса из неизменного исходного дерева; рост одной высоты не попадает в другую.

use super::{ColumnStack, rebuild};
use crate::layout::fragment::paginate::watch::Watched;
use crate::layout::fragment::plan::{self, Space};
use crate::layout::fragment::push::Regrow;
use crate::layout::fragment::token::Error;
use crate::layout::fragment::types::{Kid, StackChild};
use crate::layout::multicol::column_stack::balance::search::{Trial, min_shortage, search};
use gpui::{App, Window};
use std::collections::HashSet;

#[derive(Clone)]
struct Snapshot {
    geometry: Kid,
    regrow: Regrow,
}

impl Snapshot {
    fn take(child: &StackChild) -> Option<Self> {
        Some(Self {
            geometry: child.kid(),
            regrow: child.regrow.clone()?,
        })
    }

    fn restore(&self, child: &mut StackChild, fresh: bool) {
        let k = &self.geometry;
        child.h = k.h;
        child.over = k.over;
        child.mt = k.mt;
        child.mb = k.mb;
        child.monolith = k.monolith;
        child.cuts = k.cuts.clone();
        child.forced = k.forced.clone();
        child.solid = k.solid.clone();
        child.avoid = k.avoid.clone();
        child.keep = k.keep.clone();
        child.force_before = k.force_before;
        child.force_after = k.force_after;
        child.avoid_before = k.avoid_before;
        child.avoid_after = k.avoid_after;
        child.par = k.par;
        let mut r = self.regrow.clone();
        if fresh {
            let blocks = r.watched.blocks().expect("regrow records blocks").clone();
            r.watched = Watched::for_blocks_axis(blocks, r.axis, r.watched.flow.lines);
            r.opts.frag.watch = Some(r.watched.clone());
            r.laid = Default::default();
            r.floats.clear();
            r.float_leads.clear();
            r.space = None;
        }
        child.watched = Some(r.watched.clone());
        child.regrow = Some(r);
        rebuild(child);
    }
}

impl ColumnStack {
    pub(crate) fn regrow_children(&mut self, window: &mut Window, cx: &mut App) {
        self.reserve_row_copies();
        if !self.children.iter().any(|c| c.regrow.is_some()) {
            return;
        }
        if self.fixed_height.is_some() || self.rows.is_some_and(|r| !r.cap) {
            self.balance_failure = self.settle(window, cx).err();
            return;
        }
        let source: Vec<_> = self.children.iter().map(Snapshot::take).collect();
        let kids = self.kid_geoms();
        let (start, _, initial, _) = self.balance(&kids);
        if let Some(error) = initial.failure {
            self.balance_failure = Some(error);
            return;
        }
        let cap = self.rows.and_then(|r| r.h).map(|h| h.max(1.));
        let ceiling = cap.unwrap_or_else(|| {
            self.oof_balance_bound(Self::whole_flow_bound(&kids))
                .max(start)
        });
        let forced = Self::forced_breaks(&kids).saturating_add(1);
        let chosen = search(start, ceiling, cap.is_some(), |height| {
            self.forced_h = Some(height);
            self.restore_trial(&source, true, window, cx);
            let settled = self.settle(window, cx);
            let kids = self.kid_geoms();
            let (cols, shortage, plan) =
                Self::fill_avoiding(&kids, &|_| height, Space::columns(usize::MAX));
            let avoid = (forced < self.count)
                .then(|| Self::first_avoid_violation(&kids, &plan))
                .flatten()
                .and_then(|bad| Self::avoid_shortage(&kids, &plan, bad, height));
            let oof = self.oof_balance_trial(&plan, height, self.count.max(forced));
            let settled = settled.and(oof.as_ref().map(|_| ()).map_err(|e| *e));
            Trial {
                fits: oof.as_ref().is_ok_and(|r| r.0)
                    && settled.is_ok()
                    && avoid.is_none()
                    && plan.completion == plan::Completion::Complete
                    && cols <= self.count.max(forced),
                shortage: min_shortage([avoid, oof.ok().and_then(|r| r.1), Some(shortage)]),
                value: (
                    settled,
                    self.children.iter().map(Snapshot::take).collect::<Vec<_>>(),
                ),
            }
        });
        match chosen {
            Ok((height, (Ok(()), state))) => {
                self.forced_h = Some(height);
                self.restore_trial(&state, false, window, cx);
                self.reserve_row_copies();
            }
            Ok((_, (Err(error), _))) | Err(error) => self.balance_failure = Some(error),
        }
    }

    fn restore_trial(
        &mut self,
        source: &[Option<Snapshot>],
        fresh: bool,
        window: &mut Window,
        cx: &mut App,
    ) {
        for (index, snapshot) in source.iter().enumerate() {
            if let Some(snapshot) = snapshot {
                snapshot.restore(&mut self.children[index], fresh);
                self.lay_out_main(index, window, cx);
            }
        }
    }

    /// Фиксированная высота, конечные исходные адреса и конечные f32 координаты.
    /// Повтор числового состояния — ошибка прогресса, а не принятие последней пробы.
    fn settle(&mut self, window: &mut Window, cx: &mut App) -> Result<(), Error> {
        let mut seen = HashSet::new();
        loop {
            let state = super::progress::state(self)?;
            if !self.grow_once(window, cx) {
                return Ok(());
            }
            if !seen.insert(state) {
                return Err(Error::NoProgress);
            }
        }
    }
}
