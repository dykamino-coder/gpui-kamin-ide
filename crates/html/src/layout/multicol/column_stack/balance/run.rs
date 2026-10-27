//! Баланс последнего ряда при неизменном префиксе фрагментаинеров.
use super::{ColumnStack, Plan, Space, failed, plan, search};
use crate::layout::fragment::types::Kid;

impl ColumnStack {
    /// Пробег линий колонок между спаннерами: первая линия высотой `first`
    /// (остаток текущего ряда), остальные — `cap`; последняя при
    /// `balance_last` балансируется (Blink балансирует КАЖДУЮ линию и режет
    /// её остатком ряда, так что полные ряды выходят ровно в `column-height`,
    /// а последняя — по содержимому: `column-height-003` — 80px остатка в
    /// двух колонках по 40, а не 50 + 30). `not_at_start` — сколько первых
    /// колонок стоят не с верха фрагментаинера (`Space`). Возвращает число линий, план
    /// (колонки от нуля, дети от нуля) и высоту последней линии.
    pub(crate) fn balance_run(
        &self,
        kids: &[Kid],
        first: f32,
        cap: f32,
        balance_last: bool,
        not_at_start: usize,
    ) -> (usize, Plan, f32) {
        let count = self.count;
        let space = Space::columns(usize::MAX).with_not_at_start(not_at_start);
        let line_h = |l: usize| if l == 0 { first } else { cap };
        let (cols, _, plan) = plan::fill_at(kids, &|c| line_h(c / count), space);
        let n = cols.div_ceil(count).max(1);
        let last = line_h(n - 1);
        if !balance_last {
            return (n, plan, last);
        }
        let full = (n - 1) * count;
        let rem: f32 = plan.iter().filter(|f| f.col >= full).map(|f| f.h).sum();
        let tallest = plan
            .iter()
            .filter(|f| f.col >= full && kids[f.kid].monolith)
            .map(|f| f.h)
            .fold(0.0f32, f32::max);
        let t = (rem / count as f32).max(tallest).max(1.0);
        if t >= last {
            return (n, plan, last);
        }
        let result = search::search(t, last, true, |height| {
            let at = |c: usize| if c < full { line_h(c / count) } else { height };
            let (cols, shortage, slots) = Self::fill_avoiding(kids, &at, space);
            search::Trial {
                fits: cols <= full + count && slots.completion == plan::Completion::Complete,
                shortage: (shortage < f32::MAX).then_some(shortage),
                value: slots,
            }
        });
        match result {
            Ok((height, plan)) => (n, plan, height),
            Err(error) => (n, failed(error), t),
        }
    }
}
