//! Возможности разрыва разложенной сетки (css-grid-1 §12 Fragmenting Grid Layout; Blink
//! `GridLayoutAlgorithm::PlaceGridItemsForFragmentation`). Элементы — параллельные потоки
//! своих рядов (css-grid-2 §12: разрыв внутри элемента не разрывает соседей): сетке
//! принадлежат их точки и первая неделимая часть каждого, остальное — потоку элемента, чей
//! рост считает `push::flex_flow`. Разрыв перед элементом, чьё первое содержимое не делится,
//! — это перенос его РЯДА: такие диапазоны начинаются с начала ряда.
//! Принудительные разрывы элементов переходят на границы их рядов (`break-before` — на начало
//! первого ряда, `break-after` — на конец последнего), разрыв съедает зазор перед рядом
//! (css-gaps-1 §fragmentation: зазор, совпавший с разрывом, исчезает). Граница рядов под
//! `break-*: avoid` даёт диапазон запрета, достижимый только переносом ряда: перенос уходит к
//! ближайшей границе выше без запрета, если она в том же фрагментаинере (Blink ищет вверх
//! ряд с `EBreakBetween::kAuto`, пока смещение ряда во фрагменте положительно).

use super::geometry::{BlockBreaks, walk};
use super::{GridBox, Laid};
use crate::layout::fragment::flow::FlowGeometry;
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
            let placement = laid.item_rows(it.id);
            let mut flow = BlockBreaks::default();
            if it.monolith {
                flow.hold((t, b));
            } else if let Some(inner) = &it.inner {
                walk(inner, t, b, laid, &mut flow);
            }
            // Первое содержимое элемента не делится: разрыв перед ним
            // переносит ряд целиком (`MovePastBreakpoint` → breakpoint row).
            // Монолиты элемента разрезать нельзя никаким разрывом сетки: при
            // переполнении ими растягивается фрагмент сетки. Строки, принудительные
            // разрывы и запреты — потока самого элемента (css-grid-2 §12: разрыв
            // внутри элемента не разрывает соседей); его рост считает Regrow элемента.
            let row_top = placement
                .and_then(|(r0, _)| rows.as_deref()?.get(r0).map(|r| r.0))
                .unwrap_or(t);
            let flow = flow.finish(FlowGeometry::default(), f32::MAX);
            let start = |s: (f32, f32)| {
                if s.0 <= t + COORD_EPS {
                    (row_top.min(s.0), s.1)
                } else {
                    s
                }
            };
            let first = |s: &(f32, f32)| (s.0 <= t + COORD_EPS).then(|| start(*s));
            out.solid.extend(flow.solid.iter().filter_map(first));
            out.solid.extend(flow.barriers.iter().map(|&s| start(s)));
            out.barriers.extend(flow.barriers.iter().map(|&s| start(s)));
            out.cuts.extend(flow.cuts);
            // `break-inside: avoid` внутри элемента переносит ряд, а orphans/widows его
            // абзацев ослабляются внутри самого элемента (`flex_flow`).
            out.keep.extend_from_slice(&flow.avoid_inside);
            out.avoid_inside.extend(flow.avoid_inside);
            last = Some(last.map_or(b + it.mb, |l: f32| l.max(b + it.mb)));
            if let Some((r0, r1)) = placement {
                placed.push((it, r0, r1));
            }
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
            // Место переноса ряда (рост фрагментации) — не промежуток: его не усекают.
            let gap = prev.1 + laid.growth_after(grid, i - 1);
            if next.0 > gap + COORD_EPS {
                out.cuts.push((gap, next.0));
                out.hold((gap, next.0 + 0.05));
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
