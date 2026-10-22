//! Возможности разрыва разложенной сетки (css-grid-1 §12 Fragmenting Grid Layout; Blink
//! `GridLayoutAlgorithm::PlaceGridItemsForFragmentation`). Элементы — параллельные потоки
//! своих рядов: их точки и монолиты объединяются. Разрыв перед элементом, чьё первое
//! содержимое не делится, — это перенос его РЯДА: такие диапазоны начинаются с начала ряда.
//! Принудительные разрывы элементов переходят на границы их рядов (`break-before` — на начало
//! первого ряда, `break-after` — на конец последнего), разрыв съедает зазор перед рядом
//! (css-gaps-1 §fragmentation: зазор, совпавший с разрывом, исчезает). Граница рядов под
//! `break-*: avoid` даёт диапазон запрета, достижимый только переносом ряда: перенос уходит к
//! ближайшей границе выше без запрета, если она в том же фрагментаинере (Blink ищет вверх
//! ряд с `EBreakBetween::kAuto`, пока смещение ряда во фрагменте положительно).

use super::geometry::{BlockBreaks, walk};
use super::{GridBox, Laid, item_rows};
use crate::tolerance::COORD_EPS;

/// Граница рядов: что на ней требуют элементы.
#[derive(Clone, Copy, PartialEq)]
enum Between {
    Auto,
    Forced,
    Avoid,
}

impl GridBox {
    /// Точки разрыва сетки `grid` с верхом содержимого `floor`; возвращает
    /// конец содержимого (последний ряд или нижний край элементов).
    pub(super) fn breaks(
        &self,
        grid: u64,
        floor: f32,
        laid: &Laid,
        out: &mut BlockBreaks,
    ) -> Option<f32> {
        let rows = laid.rows(grid);
        let mut placed = Vec::with_capacity(self.items.len());
        let mut last: Option<f32> = None;
        for it in &self.items {
            let Some((t, b)) = laid.at(it.id) else {
                continue;
            };
            let (r0, r1) = rows
                .as_deref()
                .map_or((0, 0), |r| item_rows(r, t - it.mt, b + it.mb, it.span));
            let mark = out.solid.len();
            if it.monolith {
                out.solid.push((t, b));
            } else if let Some(inner) = &it.inner {
                walk(inner, t, b, laid, out);
            }
            // Первое содержимое элемента не делится: разрыв перед ним
            // переносит ряд целиком (`MovePastBreakpoint` → breakpoint row).
            if let Some(row_top) = rows.as_deref().and_then(|r| r.get(r0)).map(|r| r.0) {
                for s in &mut out.solid[mark..] {
                    if s.0 <= t + COORD_EPS && row_top < s.0 {
                        s.0 = row_top;
                    }
                }
            }
            last = Some(last.map_or(b + it.mb, |l: f32| l.max(b + it.mb)));
            placed.push((it, r0, r1));
        }
        let Some(rows) = rows.filter(|r| !r.is_empty()) else {
            return last;
        };
        // `between[i]` — граница перед рядом `i` (Blink `row_break_between`).
        let mut between = vec![Between::Auto; rows.len()];
        for (i, b) in between.iter_mut().enumerate() {
            let force = placed.iter().any(|(it, r0, r1)| {
                (*r0 == i && it.force_before) || (*r1 + 1 == i && it.force_after)
            });
            let avoid = placed.iter().any(|(it, r0, r1)| {
                (*r0 == i && it.avoid_before) || (*r1 + 1 == i && it.avoid_after)
            });
            *b = if force {
                Between::Forced
            } else if avoid {
                Between::Avoid
            } else {
                Between::Auto
            };
        }
        for i in 1..rows.len() {
            let (prev, next) = (rows[i - 1], rows[i]);
            if next.0 > prev.1 + COORD_EPS {
                out.cuts.push((prev.1, next.0));
                out.solid.push((prev.1, next.0 + 0.05));
            }
            match between[i] {
                Between::Forced => out.forced.push(next.0),
                Between::Avoid => {
                    if let Some(j) = (0..i).rev().find(|&j| between[j] == Between::Auto)
                        && rows[j].0 > floor - COORD_EPS
                    {
                        out.avoid.push((rows[j].0, next.0 + 0.05));
                    }
                }
                Between::Auto => {}
            }
        }
        rows.last().map(|r| r.1).or(last)
    }
}
