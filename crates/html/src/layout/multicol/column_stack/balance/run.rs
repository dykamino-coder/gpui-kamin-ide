//! Баланс последнего ряда при неизменном префиксе фрагментаинеров.
use super::{ColumnStack, Plan, Space, failed, plan, search};
use crate::layout::fragment::types::Kid;

impl ColumnStack {
    /// Колонки стопки: `limit` фрагментаинеров шириной в колонку (строчный размер
    /// фрагментаинера читает выбор разрыва).
    pub(crate) fn column_space(&self, limit: usize) -> Space {
        Space::columns(limit).with_inline_size(self.col_w.get())
    }

    /// Высота линии `l` пробега: первая — `first`, остальные — `cap`, но не за
    /// остаток коробки `room` от начала пробега (Blink `ConstrainColumnBlockSize`).
    pub(crate) fn run_line(l: usize, first: f32, cap: f32, room: Option<f32>) -> f32 {
        let base = if l == 0 { first } else { cap };
        let at = if l == 0 {
            0.0
        } else {
            first + (l - 1) as f32 * cap
        };
        room.map_or(base, |r| base.min(r - at)).max(1.0)
    }

    /// Пробег линий колонок между спаннерами: первая линия высотой `first`
    /// (остаток текущего ряда), остальные — `cap`, не дальше остатка коробки
    /// `room`; последняя при
    /// `balance_last` балансируется (Blink балансирует КАЖДУЮ линию и режет
    /// её остатком ряда, так что полные ряды выходят ровно в `column-height`,
    /// а последняя — по содержимому: `column-height-003` — 80px остатка в
    /// двух колонках по 40, а не 50 + 30). `not_at_start` — сколько первых
    /// колонок стоят не с верха фрагментаинера (`Space`). Возвращает число линий, план
    /// (колонки от нуля, дети от нуля) и высоту последней линии.
    pub(crate) fn balance_run(
        &self,
        kids: &[Kid],
        (first, cap, room): (f32, f32, Option<f32>),
        balance_last: bool,
        not_at_start: usize,
    ) -> (usize, Plan, f32) {
        let count = self.count;
        let space = self
            .column_space(usize::MAX)
            .with_not_at_start(not_at_start);
        let line_h = |l: usize| Self::run_line(l, first, cap, room);
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

#[cfg(test)]
mod tests {
    use super::ColumnStack;

    /// Rows of a nested multicol stop at its used block size (Blink
    /// `ConstrainColumnBlockSize`; `balanced-inner-multicol`: `max-height:
    /// 260px` in 100px outer columns gives rows 100, 100 and 60).
    #[test]
    fn nested_rows_stop_at_the_box_block_size() {
        let line = |l| ColumnStack::run_line(l, 100., 100., Some(260.));
        assert_eq!((line(0), line(1), line(2)), (100., 100., 60.));
        assert_eq!(ColumnStack::run_line(2, 100., 100., None), 100.);
        // A box already used up still keeps a 1px fragmentainer.
        assert_eq!(line(3), 1.);
    }
}
