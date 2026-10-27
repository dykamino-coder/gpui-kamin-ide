//! Независимая OOF-проба: новые потоки и token store для каждой высоты колонок.

use super::{ColumnStack, Error, Plan, Space, Streams};

impl ColumnStack {
    pub(crate) fn oof_balance_bound(&self, normal: f32) -> f32 {
        self.oof
            .geometry
            .flows
            .iter()
            .filter(|source| {
                source.policy
                    != crate::layout::fragment::token::resume::oof::FixedPolicy::RepeatPages
            })
            .fold(normal, |bound, source| {
                let start = crate::layout::fragment::flow::coordinates::block_offset(
                    source.owner_rect.origin,
                    source.owner_rect.size,
                    source.metrics.normal_rect().origin,
                    source.metrics.normal_rect().size,
                    self.axis,
                );
                bound.max(normal + start.max(0.) + source.kid.h.max(source.kid.over))
            })
    }

    pub(crate) fn oof_balance_trial(
        &self,
        normal: &Plan,
        height: f32,
        columns: usize,
    ) -> Result<(bool, Option<f32>), Error> {
        let (_, inputs) = self.oof_inputs(normal)?;
        if inputs.is_empty() {
            return Ok((true, None));
        }
        let mut streams = Streams::new(inputs)?;
        let mut shortage: Option<f32> = None;
        let mut overflowing = false;
        for col in 0..columns {
            let batch = streams.resume(
                col,
                Space {
                    wm: self.oof.wm,
                    inline_size: Some(self.col_w.get()),
                    block_size: Some(height),
                    ..Space::columns()
                },
                col + 1 == columns,
            )?;
            if let Some(s) = streams.shortage() {
                shortage = Some(shortage.map_or(s, |old| old.min(s)));
            }
            // Монолит с верха пустой колонки обязан разместиться, даже
            // переполнив её. Завершение такого потока ещё не означает баланс.
            for (_, fragment) in batch.fragments {
                let excess = fragment.y + fragment.h - height;
                if excess > 0. {
                    overflowing = true;
                    shortage = Some(shortage.map_or(excess, |old| old.min(excess)));
                }
            }
            if !streams.needs_space() {
                return Ok((!overflowing, shortage));
            }
        }
        Ok((false, shortage))
    }
}
