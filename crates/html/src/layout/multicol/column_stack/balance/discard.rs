//! Отбрасывание переполнения многоколоночника без рядов (`continue: discard`): линии
//! колонок между спаннерами укладываются одним проходом, и первая линия с переполняющими
//! колонками — последнее, что выводится (css-overflow-4 §discarding-content).

use crate::layout::fragment::plan::Plan;
use crate::layout::fragment::types::{Frag, Kid};
use crate::layout::multicol::column_stack::ColumnStack;

impl ColumnStack {
    /// Линии без рядов (`cap` — потолок баланса коробки) при `continue: discard`. Каждый пробег между спаннерами —
    /// своя сбалансированная линия (css-multicol-1 §6.1; Blink
    /// `ColumnLayoutAlgorithm::LayoutChildren` → `LayoutRow` / `LayoutSpanner`),
    /// спаннер — сразу за ней. Колонки сверх `column-count` в линии не выводятся,
    /// как и всё, что идёт после них: следующие спаннеры, линии и внепоточные
    /// потомки (css-overflow-4: «nor is any subsequent content … nor is any
    /// additional multi-column line or multi-column row»). Колонка `col` линии
    /// `l` получает номер `l · count + col` (`ColumnStack::wraps`).
    #[allow(clippy::type_complexity)]
    pub(super) fn balance_discard(
        &self,
        kids: &[Kid],
    ) -> (f32, Vec<(f32, f32)>, Plan, Vec<(usize, f32)>) {
        let count = self.count;
        // Потолок баланса (`Rows::cap`) — та же единственная сетка линий.
        let cap = self.rows.and_then(|r| r.h).map(|lim| lim.max(1.0));
        let limit = if cap.is_some() {
            self.copies
        } else if self.fixed_height.is_some() {
            self.copies.max(count)
        } else {
            count
        };
        let mut y = 0.0f32;
        let mut lines: Vec<(f32, f32)> = Vec::new();
        let mut plan = Plan::default();
        let mut spans: Vec<(usize, f32)> = Vec::new();
        let single = !kids.iter().any(|k| k.span);
        let mut i = 0usize;
        while i < kids.len() {
            if kids[i].span {
                // Поля спаннера не схлопываются с линиями колонок: ряд — свой
                // контекст форматирования (§column-span).
                let k = &kids[i];
                let at = y + k.mt;
                spans.push((i, at));
                y = at + k.h + k.mb;
                i += 1;
                continue;
            }
            let j = kids[i..]
                .iter()
                .position(|k| k.span)
                .map_or(kids.len(), |p| i + p);
            let (h, frags) = self.balance_line(&kids[i..j], limit, cap);
            plan.failure = plan.failure.or(frags.failure);
            let base = lines.len() * count;
            let mut overflow = false;
            for f in frags.iter() {
                if f.col >= count {
                    overflow = true;
                    continue;
                }
                plan.push(Frag {
                    kid: f.kid + i,
                    col: f.col + base,
                    ..*f
                });
            }
            if !overflow
                && frags.completion == crate::layout::fragment::plan::Completion::Incomplete
            {
                plan.incomplete();
            }
            // Коробка по самой высокой занятой колонке (`fill_shrink`) — только у
            // единственной линии, как без отбрасывания.
            let h = if self.fill_shrink && single {
                h.min(plan.iter().map(|f| f.y + f.h).fold(0.0f32, f32::max))
            } else {
                h
            };
            lines.push((y, h));
            y += h;
            if overflow {
                break;
            }
            i = j;
        }
        (y, lines, plan, spans)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kid(h: f32, force_after: bool) -> Kid {
        Kid {
            h,
            over: h,
            force_after,
            ..Default::default()
        }
    }

    fn spanner(h: f32) -> Kid {
        Kid {
            span: true,
            ..kid(h, false)
        }
    }

    fn stack() -> ColumnStack {
        ColumnStack::new(vec![], 3, 0.0, None, None, None, None, Default::default())
            .with_discard(true)
    }

    #[test]
    fn a_line_with_overflow_columns_ends_the_output() {
        // Четыре принудительных колонки в трёх: четвёртая переполняет, спаннер
        // и линия за ней отбрасываются.
        let kids = [
            kid(10.0, true),
            kid(10.0, true),
            kid(10.0, true),
            kid(10.0, true),
            spanner(10.0),
            kid(10.0, false),
        ];
        let (h, lines, plan, spans) = stack().balance(&kids);
        assert_eq!(h, 10.0);
        assert_eq!(lines, vec![(0.0, 10.0)]);
        assert!(spans.is_empty());
        let cols: Vec<_> = plan.iter().map(|f| (f.kid, f.col)).collect();
        assert_eq!(cols, vec![(0, 0), (1, 1), (2, 2)]);
    }

    #[test]
    fn lines_without_overflow_keep_their_spanners() {
        let kids = [
            kid(10.0, true),
            kid(10.0, false),
            spanner(5.0),
            kid(20.0, false),
        ];
        let (h, lines, plan, spans) = stack().balance(&kids);
        assert_eq!(spans, vec![(2, 10.0)]);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1].0, 15.0);
        assert!(plan.iter().any(|f| f.kid == 3 && f.col == 3));
        assert_eq!(h, 15.0 + lines[1].1);
    }
}
