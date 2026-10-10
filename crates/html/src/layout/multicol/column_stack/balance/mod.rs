//! Балансировка колонок.
// owner: A

use crate::layout::fragment::types::{Frag, Kid, NotTop, Rows};
use crate::layout::multicol::column_stack::ColumnStack;
mod growth;
mod line;

impl ColumnStack {
    /// Укладка стопки: её высота, линии колонок `(y, высота)`, план кусков и
    /// спаннеры `(ребёнок, y)`. Без рядов — одна линия, как в css-multicol-1
    /// (прежний `balance`).
    /// ★ ЗАМЕРЕНО И ОТКАЧЕНО (10.09, v205/v206, `scout-fragoof-2026-09d.md`):
    /// оба патча захода по внепоточным в многоколоночнике.
    /// №2 «переполняющие колонки» (число копий не упирается в `column-count`
    /// при `column-fill: auto` с заданной высотой, css-multicol-1 §Overflow):
    /// вместе с №1 срез 826 пар дал **+18/−31**.
    /// №1 «статическая позиция прямого внепоточного ребёнка» (щуп нулевой
    /// записью стопки, коробка заместителем `spot_place` после стопки):
    /// в одиночку **+11/−17**.
    /// Потери у обоих одни и те же и лежат в БАЛАНСИРОВКЕ:
    /// `column-height-002/003/004/021/022`, `multicol-containing-003`,
    /// `multicol-fill-balance-038` уходят в «красное видно»,
    /// `multi-line-*-flex-fragmentation-*` разъезжаются. Нулевая запись стопки
    /// и лишние копии одинаково сбивают план балансировки: она считает
    /// содержимое по числу записей, а внепоточная в них не участвует.
    /// Возвращать только вместе с разделением «переполнение» и
    /// «балансировка» в самой `balance`.
    pub(crate) fn balance(
        &self,
        kids: &[Kid],
    ) -> (f32, Vec<(f32, f32)>, Vec<Frag>, Vec<(usize, f32)>) {
        let count = self.count;
        // Потолок баланса без рядов (`Rows::cap`): баланс как прежде, но не
        // выше высоты коробки; копий — сколько построил `render.rs`, лишние
        // колонки переполняют вбок (`place`: `(col, 0.0)`). Линия — высотой
        // в баланс, не в потолок: по ней `growths` меряет рост.
        if let Some(Rows {
            h: Some(lim),
            cap: true,
            ..
        }) = self.rows
        {
            let (h, plan) = self.balance_line(kids, self.copies, Some(lim.max(1.0)));
            return (h, vec![(0.0, h)], plan, Vec::new());
        }
        let Some(rows) = self.rows else {
            // Предел копий поднимает ТОЛЬКО `column-fill: auto` с заданной
            // высотой: там `balance_line` первой строкой уходит в
            // `fill_avoiding` и высоту не подбирает. У балансировки предел
            // остаётся `count` — её условие выхода `cols <= self.count`
            // (Blink `ResolveColumnAutoBlockSize`) от числа копий зависеть не
            // должно.
            let limit = if self.fixed_height.is_some() {
                self.copies.max(count)
            } else {
                count
            };
            let (h, plan) = self.balance_line(kids, limit, None);
            return (h, vec![(0.0, h)], plan, Vec::new());
        };
        let limit = self.copies;
        let Some(h) = rows.h else {
            // Ряд без потолка: колонки балансируются одной линией, а лишние
            // (от принудительных разрывов) идут рядами ниже; высота ряда — по
            // его содержимому (Blink: `HasRowHeight()` ложь, `OffsetToNextRow`
            // = один `row_gap`; `column-wrap-no-constraints-001/002`).
            let (_, plan) = self.balance_line(kids, limit, None);
            let n = plan.iter().map(|f| f.col / count + 1).max().unwrap_or(1);
            let mut out = Vec::with_capacity(n);
            let mut y = 0.0f32;
            for r in 0..n {
                let rh = plan
                    .iter()
                    .filter(|f| f.col / count == r)
                    .map(|f| f.y + f.h)
                    .fold(0.0f32, f32::max);
                out.push((y, rh));
                y += rh + if r + 1 < n { rows.gap } else { 0.0 };
            }
            return (y, out, plan, Vec::new());
        };
        // Blink `ClampedToValidFragmentainerCapacity`: нулевая колонка всё
        // равно вмещает 1px, иначе укладка не сдвинется с места
        // (`columns: 2 / 0`, `column-height-021…023`).
        let cap = h.max(1.0);
        if !rows.wrap {
            // `nowrap` с заданным `column-height`: одна линия высотой ровно в
            // него, баланс не выше потолка (Blink `ConstrainColumnBlockSize`:
            // «Never become taller than used column-height»), лишние
            // колонки — вбок (§8.2 уровня 1; `column-height-005/030`).
            let target = match self.fixed_height {
                Some(_) => cap,
                None => self.balance_line(kids, limit, Some(cap)).0,
            };
            let (_, _, plan) = Self::fill(kids, target, limit, false);
            return (h, vec![(0.0, h)], plan, Vec::new());
        }
        // Курсор по коробке (Blink `intrinsic_block_size_`): сетка рядов с
        // шагом `h + row-gap`; фаза курсора — смещение в текущем ряду
        // (`OffsetInCurrentRow`), остаток ряда `h − фаза` (отрицателен в
        // зазоре). Дети идут ПРОБЕГАМИ: линии колонок между спаннерами и сами
        // спаннеры (`LayoutChildren` → `LayoutFragmentationContext` /
        // `LayoutSpanner`). Без спаннеров — один пробег, план шага 1.
        let stride = h + rows.gap;
        let phase = |y: f32| if stride > 0.0 { y % stride } else { 0.0 };
        // `OffsetToNextRow`: остаток ряда + зазор; ровно с начала ряда — один
        // зазор (так у Blink; на практике сюда не попадаем).
        let next_row = |y: f32| {
            let p = phase(y);
            if p > 0.01 {
                y - p + stride
            } else {
                y + rows.gap
            }
        };
        // Начало рядов со сдвигом `row_phase`: курсор встаёт на фазу внутри
        // первого ряда, и его остаток — первая линия (`balance_run(first)`);
        // в конце координаты возвращаются к верху коробки.
        let phase0 = if h > 0.0 {
            self.row_phase.min(h - 0.01).max(0.0)
        } else {
            0.0
        };
        let _not_top = NotTop::set(if phase0 > 0.01 { count } else { 0 });
        let mut y = phase0;
        let mut lines: Vec<(f32, f32)> = Vec::new();
        let mut plan: Vec<Frag> = Vec::new();
        let mut spans: Vec<(usize, f32)> = Vec::new();
        let mut i = 0usize;
        while i < kids.len() {
            if kids[i].span {
                // Спаннер (§ch: выше ряда — переполняет в следующий, «crossing
                // any row-gap»). Не с начала ряда и не влезает в остаток —
                // со следующего ряда (`LayoutSpanner`: «Not enough room for
                // the spanner in the current row, and we're not at the
                // beginning of the row. Try at the next row»). Поля спаннера —
                // без схлопывания с соседями (упрощение; в тестах кластера
                // нулевые).
                let k = &kids[i];
                let mut at = y + k.mt;
                let p = phase(at);
                if p > 0.01 && h - p < k.h - 0.01 {
                    at = next_row(at);
                }
                spans.push((i, at));
                y = at + k.h + k.mb;
                i += 1;
                continue;
            }
            let j = kids[i..]
                .iter()
                .position(|k| k.span)
                .map_or(kids.len(), |p| i + p);
            // Первая линия пробега — в остаток текущего ряда; остатка нет
            // (курсор в зазоре после спаннера) — со следующего ряда
            // (`LayoutFragmentationContext`: «if there's no room in the
            // current row (because of a preceding spanner, typically)»;
            // нулевой ряд — исключение, `RowHeight() > LayoutUnit()`).
            let mut p = phase(y);
            if h > 0.0 && h - p <= 0.01 {
                y = next_row(y);
                p = 0.0;
            }
            let first = (h - p).max(1.0);
            let row_start = y - p;
            // Линия перед спаннером балансируется всегда, даже при
            // `column-fill: auto` (Blink `:1070`: «We always have to balance
            // columns preceding a spanner»; css-gaps `multicol-gap-
            // decorations-011`: 120px в трёх колонках по 40, а не 60 + 60).
            let balance_last = self.fixed_height.is_none() || j < kids.len();
            let base = lines.len() * count;
            let (n, frags, tail) = self.balance_run(&kids[i..j], first, cap, balance_last);
            for f in frags {
                plan.push(Frag {
                    kid: f.kid + i,
                    col: f.col + base,
                    ..f
                });
            }
            for l in 0..n {
                let ly = if l == 0 {
                    y
                } else {
                    row_start + l as f32 * stride
                };
                let lh = if l + 1 == n {
                    tail
                } else if l == 0 {
                    first
                } else {
                    cap
                };
                lines.push((ly, lh));
            }
            // Курсор — за последней линией: сбалансированная короче ряда, и
            // следующий спаннер ложится прямо за ней (Blink `:1249`).
            y = lines.last().map_or(y, |&(ly, lh)| ly + lh);
            i = j;
        }
        // Последний ряд занимает всю `column-height`, даже если содержимое
        // короче (§ch: «empty space is left»; Blink `Layout()`: «Use all of
        // column-height on the last row as well» — прибавляется и
        // отрицательный остаток: `columns: 2 / 0` даёт `(n − 1) · gap`, как в
        // шаге 1).
        let p = phase(y);
        if p > 0.01 {
            y += h - p;
        }
        for l in lines.iter_mut() {
            l.0 -= phase0;
        }
        for s in spans.iter_mut() {
            s.1 -= phase0;
        }
        (y - phase0, lines, plan, spans)
    }
}
