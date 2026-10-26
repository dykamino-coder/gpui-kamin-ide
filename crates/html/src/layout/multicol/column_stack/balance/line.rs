//! Балансировка по строкам: строка баланса, avoid на пределе, прогон.

use crate::layout::fragment::plan::Plan;
use crate::layout::fragment::plan::{self, Space};
use crate::layout::fragment::types::Kid;
use crate::layout::multicol::column_stack::ColumnStack;
use crate::tolerance::COORD_EPS;

impl ColumnStack {
    /// Одна линия колонок: высота заданная (fill:auto) либо баланс «оценка +
    /// добавка на минимальный недолаз» (blink `ResolveColumnAutoBlockSize`);
    /// `cap` — потолок баланса (`ConstrainColumnBlockSize`).
    pub(crate) fn balance_line(&self, kids: &[Kid], limit: usize, cap: Option<f32>) -> (f32, Plan) {
        if let Some(h) = self.fixed_height {
            // Правило 1 css-break-4 §4.3 применяется ТОЛЬКО здесь —
            // `column-fill: auto` с заданной высотой колонки. Балансировку
            // (`ResolveColumnAutoBlockSize` ниже) и пробег рядов
            // (`balance_run`) отступ не трогает НАМЕРЕННО: там лишняя колонка
            // от отступа растит `target` на весь недолаз и МЕНЯЕТ высоту
            // многоколоночника (`balance-break-avoidance-002`: баланс уходит
            // в 125 при верных 100) — в балансе отступ мало поставить, надо
            // ещё выбрать высоту, а это отдельный шаг.
            let (_, _, slots) = Self::fill_avoiding(kids, &|_| h, Space::columns(limit));
            return (h, slots);
        }
        if let Some(h) = self.forced_h {
            let (_, _, slots) = Self::fill_avoiding(kids, &|_| h, Space::columns(usize::MAX));
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
            Some(k) if k.mb > COORD_EPS && !k.span && k.par.group == 0 => {
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
        // 037`: `contain: size` 100 при оценке 75).
        // Верхнее поле ПЕРВОГО ребёнка не примыкает к разрыву (начало контекста
        // фрагментации, css-break-3 §5.2 усекает поля только у разрыва) — монолит
        // под ним целиком в первой колонке, и Blink растит баланс на недолаз
        // (`PropagateSpaceShortage` у монолита, не влезшего в колонку): три
        // монолита 60 с `margin-top: 20px` у первого — колонки 80, а не 60.
        let tallest = kids
            .iter()
            .enumerate()
            .map(|(i, k)| {
                let normal = if k.monolith {
                    k.h + if i == 0 && k.par.group == 0 {
                        k.mt.max(0.0)
                    } else {
                        0.0
                    }
                } else {
                    k.solid.iter().fold(0.0f32, |m, &(a, b)| m.max(b - a))
                };
                normal.max(k.float_min)
            })
            .fold(0.0f32, f32::max);
        let start = guess.max(tallest).max(1.0);
        let ceiling = cap.unwrap_or_else(|| {
            self.oof_balance_bound(Self::whole_flow_bound(kids))
                .max(start)
        });
        let start = start.min(ceiling);
        let forced_columns = Self::forced_breaks(kids).saturating_add(1);
        let result = super::search::search(start, ceiling, cap.is_some(), |height| {
            // Число запасных paint-копий не ограничивает измерительную пробу.
            let (cols, shortage, slots) =
                Self::fill_avoiding(kids, &|_| height, Space::columns(usize::MAX));
            let avoid = (forced_columns < self.count)
                .then(|| Self::first_avoid_violation(kids, &slots))
                .flatten()
                .and_then(|bad| Self::avoid_shortage(kids, &slots, bad, height));
            let oof = self.oof_balance_trial(&slots, height, self.count.max(forced_columns));
            let fits = oof.as_ref().is_ok_and(|r| r.0)
                && slots.completion == plan::Completion::Complete
                && cols <= self.count.max(forced_columns)
                && avoid.is_none();
            super::search::Trial {
                fits,
                shortage: super::search::min_shortage([
                    avoid,
                    oof.as_ref().ok().and_then(|r| r.1),
                    Some(shortage),
                ]),
                value: match oof {
                    Ok(_) => slots,
                    Err(error) => super::failed(error),
                },
            }
        });
        match result {
            Ok(chosen) => chosen,
            Err(error) => (start, super::failed(error)),
        }
    }

    /// Конечная верхняя граница: все положительные протяжённости и поля
    /// исходного потока помещаются без неявных разрывов. Forced остаются.
    pub(crate) fn whole_flow_bound(kids: &[Kid]) -> f32 {
        let bound = kids
            .iter()
            .map(|k| {
                f64::from(k.h.max(k.over).max(k.float_min))
                    + f64::from(k.mt.max(0.0))
                    + f64::from(k.mb.max(0.0))
                    + f64::from(k.repeat.head + k.repeat.foot)
            })
            .sum::<f64>()
            .min(f64::from(f32::MAX));
        let rounded = bound as f32;
        if f64::from(rounded) < bound {
            rounded.next_up()
        } else {
            rounded
        }
    }
}
