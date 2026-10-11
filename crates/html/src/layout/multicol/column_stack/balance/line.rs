//! Балансировка по строкам: строка баланса, avoid на пределе, прогон.

use crate::layout::fragment::types::{Frag, Kid};
use crate::layout::multicol::column_stack::ColumnStack;

impl ColumnStack {
    /// Одна линия колонок: высота заданная (fill:auto) либо баланс «оценка +
    /// добавка на минимальный недолаз» (blink `ResolveColumnAutoBlockSize`);
    /// `cap` — потолок баланса (`ConstrainColumnBlockSize`).
    pub(crate) fn balance_line(
        &self,
        kids: &[Kid],
        limit: usize,
        cap: Option<f32>,
    ) -> (f32, Vec<Frag>) {
        if let Some(h) = self.fixed_height {
            // Правило 1 css-break-4 §4.3 применяется ТОЛЬКО здесь —
            // `column-fill: auto` с заданной высотой колонки. Балансировку
            // (`ResolveColumnAutoBlockSize` ниже) и пробег рядов
            // (`balance_run`) отступ не трогает НАМЕРЕННО: там лишняя колонка
            // от отступа растит `target` на весь недолаз и МЕНЯЕТ высоту
            // многоколоночника. Считано руками на `balance-break-avoidance-002`
            // (сегодня 0.53): баланс уходит 75 -> 125 при верных 100 — то есть
            // в балансе отступ мало поставить, надо ещё выбрать высоту, а это
            // отдельный шаг с отдельным замером.
            let (_, _, slots) = Self::fill_avoiding(kids, &|_| h, limit, false);
            return (h, slots);
        }
        // Нижнее поле ПОСЛЕДНЕГО ребёнка не примыкает к разрыву — многоколоночник
        // свой контекст форматирования (CSS 2.1 §8.3.1), и поле остаётся в
        // содержимом последней колонки: балансу оно нужно так же, как высота
        // (Blink `column_layout_algorithm.cc` `CalculateBalancedColumnBlockSize`
        // считает `content_block_size` с концевым полем). Без него колонки
        // выходили на поле короче (`multicol-nested-002`: 60 против 80). На
        // разрыве поле по-прежнему усекается — оно только у последнего.
        let tail;
        let kids = match kids.last() {
            Some(k) if k.mb > 0.01 && !k.span && k.par.group == 0 => {
                let mut v = kids.to_vec();
                if let Some(l) = v.last_mut() {
                    l.h += l.mb;
                    l.over = l.over.max(l.h);
                    l.mb = 0.0;
                }
                tail = v;
                &tail[..]
            }
            _ => kids,
        };
        // Оценка — по прогонам между принудительными разрывами (`runs_guess`); без
        // них это прежняя `сумма / count`.
        let guess = Self::runs_guess(kids, self.count);
        // Разрезаемая коробка потолка колонке не задаёт: её высоту держит
        // только сумма. Потолок нужен монолитам — они остаются целыми.
        // Баланс не короче самого длинного неразрывного куска: монолита-ребёнка
        // и монолитного диапазона (`solid`) внутри разрезаемого ребёнка (Blink
        // `column_layout_algorithm.cc` — `tallest_unbreakable_block_size` в
        // `CalculateBalancedColumnBlockSize`). Без этого монолит-потомок выше
        // оценки резался краем колонки (`single-line-row-flex-fragmentation-
        // 037`: `contain: size` 100 при оценке 75). Прежний замер (06.09, v103)
        // терял `multicol-overflow-clip`; на integration-7 она цела.
        // Верхнее поле ПЕРВОГО ребёнка не примыкает к разрыву (начало контекста
        // фрагментации, css-break-3 §5.2 усекает поля только у разрыва) — монолит
        // под ним целиком в первой колонке, и Blink растит баланс на недолаз
        // (`PropagateSpaceShortage` у монолита, не влезшего в колонку): три
        // монолита 60 с `margin-top: 20px` у первого — колонки 80, а не 60.
        let tallest = kids
            .iter()
            .enumerate()
            .map(|(i, k)| {
                if k.monolith {
                    k.h + if i == 0 && k.par.group == 0 {
                        k.mt.max(0.0)
                    } else {
                        0.0
                    }
                } else {
                    k.solid.iter().fold(0.0f32, |m, &(a, b)| m.max(b - a))
                }
            })
            .fold(0.0f32, f32::max);
        let clamp = |t: f32| cap.map_or(t, |c| t.min(c));
        let mut target = clamp(guess.max(tallest).max(1.0));
        for _ in 0..6 {
            let (cols, shortage, slots) = Self::fill(kids, target, limit, false);
            if cols <= self.count {
                // Колонок хватило, но план рвёт запрещённую границу (css-break-3
                // §4.3, правило 1) — растим высоту ровно на недолаз
                // (`avoid_shortage`): Blink выходит из цикла балансировки только
                // без нарушений (`column_layout_algorithm.cc:1145-1147`).
                // `balance-break-avoidance-002` 75 → 100, `-001` 50 → 100.
                //
                // Гейты — те же, что у Blink перед растяжением. (1) Рост идёт
                // через `clamp`, то есть в потолок `cap` = заданная высота коробки
                // (`Rows::cap`, `render.rs`; Blink `ConstrainColumnBlockSize`,
                // `:1172`, `:1785-1788`, `:1812`), и если выше не вышло — план
                // принимается С НАРУШЕНИЕМ (`:1175-1178` «Give up if we cannot
                // get taller columns»; css-break-3 §4.3: «rules 1, 2 and 4 are
                // dropped»). ★ Без этого гейта (замер 30.09, `Rows::cap` ещё не
                // было) `flex-/grid-lanes-container-fragmentation-003/004` —
                // `height: 100px`, 50 + 50 + 300 в четырёх колонках — росли
                // 100 → 400 за коробку. (2) Принудительных разрывов не меньше
                // `count − 1` — растягивать бесполезно (`:1152`, `forced_breaks`).
                if Self::forced_breaks(kids) + 1 < self.count
                    && let Some(d) = Self::first_avoid_violation(kids, &slots)
                        .and_then(|bad| Self::avoid_shortage(kids, &slots, bad, target))
                {
                    let grown = clamp(target + d);
                    if grown > target + 0.01 {
                        target = grown;
                        continue;
                    }
                }
                return (target, Self::avoid_at_cap(kids, target, limit, slots));
            }
            // Недолаза не было ни у одной коробки: `shortage` так и остался
            // сторожевым `f32::MAX`. Значит лишние колонки родились
            // ПРИНУДИТЕЛЬНЫМИ разрывами, и растягивать нечего —
            // css-multicol-1 §7: «minimize variations in column height, while
            // honoring forced breaks»; Blink: `if (used_column_count_ <=
            // forced_break_count + 1) break;`.
            // Прежняя строка спрашивала `shortage.is_finite()`, а `f32::MAX` —
            // КОНЕЧНОЕ число: `target += f32::MAX` уводил высоту колонки в
            // `f32::MAX`, затем в бесконечность (`multicol-fill-balance-002`,
            // «Don't overstretch»).
            // Упёрлись в потолок — выше колонкам нельзя, остаток уходит вбок.
            if shortage >= f32::MAX {
                // Лишние колонки — только от принудительных разрывов: каждая
                // колонка держит свой прогон целиком, и высота линии — самый
                // высокий из них (Blink `ContentRuns::DistributeImplicitBreaks`
                // при числе прогонов больше колонок; css-multicol-1 §7
                // «honoring forced breaks»). Стартовая оценка `total / count`
                // выше любого прогона (`grid-item-fragmentation-039`:
                // `columns: 1`, прогоны 100 + 100 — колонка 200 вместо 100).
                let used = slots.iter().map(|f| f.y + f.h).fold(0.0f32, f32::max);
                if used > 0.01 && used < target - 0.01 {
                    let (cols2, _, slots2) = Self::fill(kids, used, limit, false);
                    if cols2 == cols {
                        return (used, slots2);
                    }
                }
                return (target, slots);
            }
            if cap.is_some_and(|c| target >= c) {
                return (target, Self::avoid_at_cap(kids, target, limit, slots));
            }
            // Как blink: расти ровно на минимально необходимое.
            target = clamp(target + if shortage > 0.0 { shortage } else { 1.0 });
        }
        let (_, _, slots) = Self::fill(kids, target, limit, false);
        (target, slots)
    }

    /// Колонки упёрлись в потолок (`Rows::cap`, заданная высота коробки), а
    /// план рвёт запрещённую границу: выше расти нельзя, но окончательная
    /// укладка в колонки этой высоты — обычная фрагментация со своими ранними
    /// разрывами (css-break-3 §4.3 правило 1; Blink балансирует высоту
    /// `CalculateBalancedColumnBlockSize`, а затем раскладывает содержимое
    /// `LayoutRow` той же `block_layout_algorithm` с `early_break_`,
    /// `fragmentation_utils.cc:1250`). `flex-container-fragmentation-003/004`:
    /// 50 + 50 + 300 при `break-before: avoid` у третьей — разрыв после первой.
    pub(crate) fn avoid_at_cap(
        kids: &[Kid],
        target: f32,
        limit: usize,
        slots: Vec<Frag>,
    ) -> Vec<Frag> {
        if Self::first_avoid_violation(kids, &slots).is_none() {
            return slots;
        }
        Self::fill_avoiding(kids, &|_| target, limit, false).2
    }

    /// Пробег линий колонок между спаннерами: первая линия высотой `first`
    /// (остаток текущего ряда), остальные — `cap`; последняя при
    /// `balance_last` балансируется (Blink балансирует КАЖДУЮ линию и режет
    /// её остатком ряда, так что полные ряды выходят ровно в `column-height`,
    /// а последняя — по содержимому: `column-height-003` — 80px остатка в
    /// двух колонках по 40, а не 50 + 30). Возвращает число линий, план
    /// (колонки от нуля, дети от нуля) и высоту последней линии.
    pub(crate) fn balance_run(
        &self,
        kids: &[Kid],
        first: f32,
        cap: f32,
        balance_last: bool,
    ) -> (usize, Vec<Frag>, f32) {
        let count = self.count;
        let limit = self.copies;
        let line_h = |l: usize| if l == 0 { first } else { cap };
        let (cols, _, plan) = Self::fill_at(kids, &|c| line_h(c / count), limit, false);
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
        let mut t = (rem / count as f32).ceil().max(tallest).max(1.0);
        if t >= last {
            return (n, plan, last);
        }
        for _ in 0..6 {
            let at = |c: usize| if c < full { line_h(c / count) } else { t };
            let (cols, shortage, slots) = Self::fill_at(kids, &at, limit, false);
            if cols <= full + count {
                return (n, slots, t);
            }
            if shortage >= f32::MAX {
                break;
            }
            t += if shortage > 0.0 { shortage } else { 1.0 };
            if t >= last {
                break;
            }
        }
        (n, plan, last)
    }
}
