//! Пробы баланса из неизменного исходного дерева; рост одной высоты не попадает в другую.

use super::ColumnStack;
use super::copies::rebuild;
use crate::layout::fragment::plan;
use crate::layout::fragment::push::Regrow;
use crate::layout::fragment::token::Error;
use crate::layout::fragment::types::{Kid, StackChild};
use crate::layout::multicol::column_stack::balance::search::{
    Trial, ceil_unit, min_shortage, search_async,
};
use gpui::{App, Window};
use std::collections::HashSet;

struct Snapshot {
    geometry: Kid,
    regrow: Regrow,
}

impl Snapshot {
    fn take(child: &StackChild) -> Option<Self> {
        let source = child.regrow.as_ref()?;
        let watched = source.watched.fresh_blocks();
        let mut opts = source.opts.clone();
        opts.frag.watch = Some(watched.clone());
        Some(Self {
            geometry: child.kid(),
            regrow: Regrow {
                axis: source.axis,
                source: source.source.clone(),
                dom: source.dom.clone(),
                build: source.build.clone(),
                opts,
                laid: Default::default(),
                floats: Vec::new(),
                float_leads: Vec::new(),
                space: None,
                laid_h: source.laid_h,
                watched,
            },
        })
    }

    fn restore(&self, child: &mut StackChild) {
        crate::work::add!(MulticolTrialRestores, 1);
        restore_geometry(child, &self.geometry);
        let source = &self.regrow;
        let watched = source.watched.fresh_blocks();
        let mut opts = source.opts.clone();
        opts.frag.watch = Some(watched.clone());
        let r = Regrow {
            axis: source.axis,
            source: source.source.clone(),
            dom: source.dom.clone(),
            build: source.build.clone(),
            opts,
            laid: Default::default(),
            floats: Vec::new(),
            float_leads: Vec::new(),
            space: None,
            laid_h: source.laid_h,
            watched,
        };
        child.watched = Some(r.watched.clone());
        child.regrow = Some(r);
        rebuild(child);
    }
}

impl ColumnStack {
    pub(crate) async fn regrow_children_async(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) {
        crate::work::time!(_balance, MulticolBalanceNs);
        self.reserve_row_copies();
        if !self.children.iter().any(|c| c.regrow.is_some()) {
            return;
        }
        // Линии между спаннерами `continue: discard` балансируются каждая своим
        // поиском (`balance_discard`), как ряды: общая высота одной линии сюда не
        // подходит.
        if self.fixed_height.is_some() || self.rows.is_some_and(|r| !r.cap) || self.discard {
            self.balance_failure = self.settle_async(window, cx, stack).await.err();
            return;
        }
        let kids = self.kid_geoms();
        let (start, _, initial, _) = self.balance(&kids);
        if let Some(error) = initial.failure {
            self.balance_failure = Some(error);
            return;
        }
        // Без заявок роста числовая проба уже имеет итоговую геометрию:
        // повторное построение не добавляет сведений о разрывах.
        let height = ceil_unit(start).min(
            self.rows
                .and_then(|r| r.h)
                .map(|h| h.max(1.))
                .unwrap_or(f32::MAX),
        );
        let previous_height = self.forced_h;
        self.forced_h = Some(height);
        if self.growth_requests().iter().all(Vec::is_empty) {
            let forced = Self::forced_breaks(&kids).saturating_add(1);
            let (cols, _, plan) =
                Self::fill_avoiding(&kids, &|_| height, self.column_space(usize::MAX));
            let avoid = forced < self.count
                && Self::first_avoid_violation(&kids, &plan)
                    .is_some_and(|bad| Self::avoid_shortage(&kids, &plan, bad, height).is_some());
            if !avoid
                && cols <= self.count.max(forced)
                && plan.completion == plan::Completion::Complete
                && self
                    .oof_balance_trial(&plan, height, self.count.max(forced))
                    .is_ok_and(|r| r.0)
            {
                self.reserve_row_copies();
                return;
            }
        }
        self.forced_h = previous_height;

        let cap = self.rows.and_then(|r| r.h).map(|h| h.max(1.));
        let ceiling = cap.unwrap_or_else(|| {
            self.oof_balance_bound(Self::whole_flow_bound(&kids))
                .max(start)
        });
        let forced = Self::forced_breaks(&kids).saturating_add(1);
        if self
            .numeric_balance_async(start, ceiling, cap.is_some(), window, cx, stack)
            .await
        {
            return;
        }
        crate::work::add!(MulticolNumericRejected, 1);
        let source: Vec<_> = self.children.iter().map(Snapshot::take).collect();
        let chosen = search_async(start, ceiling, cap.is_some(), async |height| {
            gpui::with_temporary_element_arena_async(async || {
                window
                    .with_retained_layout_async(
                        &mut gpui::RetainedLayout::default(),
                        async |window| {
                            self.forced_h = Some(height);
                            self.restore_trial_async(&source, window, cx, stack).await;
                            let settled = self.settle_async(window, cx, stack).await;
                            let kids = self.kid_geoms();
                            let (cols, shortage, plan) = Self::fill_avoiding(
                                &kids,
                                &|_| height,
                                self.column_space(usize::MAX),
                            );
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
                                shortage: min_shortage([
                                    avoid,
                                    oof.ok().and_then(|r| r.1),
                                    Some(shortage),
                                ]),
                                value: settled,
                            }
                        },
                    )
                    .await
            })
            .await
        })
        .await;
        match chosen {
            Ok((height, Ok(()))) => {
                self.forced_h = Some(height);
                // Пробы хранят только числовой результат. Выбранный проход
                // материализуется в arena кадра один раз из исходного снимка.
                self.restore_trial_async(&source, window, cx, stack).await;
                self.balance_failure = self.settle_async(window, cx, stack).await.err();
                self.reserve_row_copies();
            }
            Ok((_, Err(error))) | Err(error) => {
                self.forced_h = previous_height;
                self.restore_trial_async(&source, window, cx, stack).await;
                self.balance_failure = Some(error);
            }
        }
    }

    async fn restore_trial_async(
        &mut self,
        source: &[Option<Snapshot>],
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) {
        for (index, snapshot) in source.iter().enumerate() {
            if let Some(snapshot) = snapshot {
                snapshot.restore(&mut self.children[index]);
                self.lay_out_main_async(index, window, cx, stack).await;
            }
        }
    }

    /// Фиксированная высота, конечные исходные адреса и конечные f32 координаты.
    /// Повтор числового состояния — ошибка прогресса, а не принятие последней пробы.
    async fn settle_async(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> Result<(), Error> {
        let mut seen = HashSet::new();
        loop {
            let state = super::progress::state(self)?;
            if !self.grow_once_async(window, cx, stack).await {
                return Ok(());
            }
            if !seen.insert(state) {
                return Err(Error::NoProgress);
            }
        }
    }
}

#[cfg(test)]
mod tests;

mod geometry;
pub(super) use geometry::restore_geometry;

#[cfg(test)]
impl ColumnStack {
    pub(crate) fn regrow_children(&mut self, window: &mut Window, cx: &mut App) {
        reblessive::Stack::new()
            .enter(|stack| self.regrow_children_async(window, cx, stack))
            .finish()
    }
}
