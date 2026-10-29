//! Ряды вложенного многоколоночника во внешнем фрагментаинере (css-multicol-2
//! §column-height, css-break-3 §2): линия колонок — остаток ряда, но не дальше края
//! внешнего фрагментаинера; ряд, начатый у края, продолжается в следующем, новый ряд
//! начинается в этом, только если умещается целиком, иначе зазор ряда тонет в
//! разрыве. Так у Blink (`ColumnLayoutAlgorithm::LayoutRows`: «Another row doesn't
//! fit in the outer fragmentainer. Break.», `MulticolBreakTokenData` с остатком ряда;
//! `ConstrainColumnBlockSize` ограничивает колонку остатком фрагментаинера).

use super::{ColumnStack, Plan, failed, plan, search};
use crate::layout::fragment::types::{Kid, Rows};
use crate::tolerance::COORD_EPS;

/// Линии `(y, высота)` в координатах коробки, первая — с фазы `phase` во внешнем
/// фрагментаинере высотой `outer`; ряды высотой `row` через `gap`.
pub(crate) fn outer_lines(phase: f32, outer: f32, row: f32, gap: f32, n: usize) -> Vec<(f32, f32)> {
    let outer = outer.max(1.0);
    let row = row.max(1.0);
    let mut lines = Vec::with_capacity(n);
    let mut at = phase.clamp(0.0, outer);
    let mut y = 0.0f32;
    let mut rest = row;
    while lines.len() < n {
        let left = outer - at;
        if left <= COORD_EPS {
            y += left.max(0.0);
            at = 0.0;
            continue;
        }
        let h = rest.min(left);
        lines.push((y, h));
        y += h;
        at += h;
        rest -= h;
        if rest > COORD_EPS {
            continue;
        }
        rest = row;
        if at + gap + row > outer + COORD_EPS {
            y += (outer - at).max(0.0);
            at = 0.0;
        } else {
            y += gap;
            at += gap;
        }
    }
    lines
}

impl ColumnStack {
    /// Укладка по линиям `outer_lines`; последняя при балансе — по содержимому.
    // Тот же итог, что у `balance`: высота, линии, план, спаннеры.
    #[allow(clippy::type_complexity)]
    pub(super) fn balance_outer_rows(
        &self,
        kids: &[Kid],
        rows: Rows,
        outer: f32,
    ) -> (f32, Vec<(f32, f32)>, Plan, Vec<(usize, f32)>) {
        let count = self.count.max(1);
        let row = rows.h.unwrap_or(outer);
        let total: f32 = kids
            .iter()
            .map(|k| k.h.max(k.over) + k.mt.max(0.0) + k.mb.max(0.0))
            .sum();
        // Линий с запасом: каждая вмещает хотя бы 1px на колонку.
        let n = ((total / count as f32).ceil() as usize)
            .saturating_add(kids.len() * 2 + 2)
            .min(1 << 14);
        let schedule = outer_lines(self.row_phase, outer, row, rows.gap, n);
        let not_at_start = if self.row_phase > COORD_EPS { count } else { 0 };
        let space = self
            .column_space(schedule.len() * count)
            .with_not_at_start(not_at_start);
        let line_h = |c: usize| schedule.get(c / count).map_or(row, |l| l.1);
        let (cols, _, mut plan) = plan::fill_at(kids, &line_h, space);
        let used = cols.div_ceil(count).clamp(1, schedule.len());
        let mut lines = schedule[..used].to_vec();
        if self.fixed_height.is_none() {
            // Последняя линия при `column-fill: balance` — по содержимому
            // (css-multicol-1 §7.1: «only the last fragment is balanced»).
            let full = (used - 1) * count;
            let last = lines[used - 1].1;
            let rem: f32 = plan.iter().filter(|f| f.col >= full).map(|f| f.h).sum();
            let t = (rem / count as f32).max(1.0);
            if t < last {
                let result = search::search(t, last, true, |height| {
                    let at = |c: usize| if c < full { line_h(c) } else { height };
                    let (cols, shortage, slots) = Self::fill_avoiding(kids, &at, space);
                    search::Trial {
                        fits: cols <= full + count
                            && slots.completion == plan::Completion::Complete,
                        shortage: (shortage < f32::MAX).then_some(shortage),
                        value: slots,
                    }
                });
                match result {
                    Ok((height, slots)) => {
                        plan = slots;
                        lines[used - 1].1 = height;
                    }
                    Err(error) => plan = failed(error),
                }
            }
        }
        let height = lines.last().map_or(0.0, |&(y, h)| y + h);
        (height, lines, plan, Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::outer_lines;

    #[test]
    fn rows_stop_at_the_outer_fragmentainer_and_resume_in_the_next() {
        // column-height-027: rows 25 with a 10px gap in outer columns of 100.
        assert_eq!(
            outer_lines(0., 100., 25., 10., 4),
            vec![(0., 25.), (35., 25.), (70., 25.), (100., 25.)]
        );
        // column-height-025: a 350px row in outer columns of 100 continues in
        // the next ones; the next row does not fit after 50px and starts fresh.
        assert_eq!(
            outer_lines(0., 100., 350., 13., 5),
            vec![
                (0., 100.),
                (100., 100.),
                (200., 100.),
                (300., 50.),
                (400., 100.)
            ]
        );
        // A row started below the top of the first outer fragmentainer.
        assert_eq!(
            outer_lines(60., 100., 30., 0., 2),
            vec![(0., 30.), (40., 30.)]
        );
    }
}
