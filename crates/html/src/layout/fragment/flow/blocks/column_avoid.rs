//! Ранний разрыв цепочки flex-элементов с break-before/after: avoid.
//! Запрет остаётся мягким: css-break-3 §4.4 разрешает ослабить правило 1,
//! когда вся цепочка не помещается. Пустой хвост после последней строки —
//! возможный разрез, но не продолжение, которое можно перенести раньше.
use super::{BlockBreaks, BlockKid, Laid};
use crate::tolerance::COORD_EPS;

#[derive(Default)]
pub(super) struct ColumnAvoid {
    start: Option<f32>,
    floor: Option<f32>,
}

impl ColumnAvoid {
    pub(super) fn visit(&mut self, top: f32) {
        self.floor.get_or_insert(top);
    }

    pub(super) fn reset(&mut self) {
        self.start = None;
    }

    pub(super) fn boundary(
        &mut self,
        previous: &BlockKid,
        (top, bottom, next): (f32, f32, f32),
        laid: &Laid,
        out: &mut BlockBreaks,
    ) {
        let start = self.start.unwrap_or(top);
        // Ранний разрыв должен переносить реальное продолжение строки или
        // блока. Последняя строка фиксированной коробки не оставляет такого
        // продолжения в её пустом хвосте (Blink UpdateEarlyBreakBetweenLines).
        let open = out
            .cuts
            .iter()
            .filter(|&&(need, resume)| {
                need > top + COORD_EPS
                    && need < bottom - COORD_EPS
                    && previous
                        .inner
                        .as_ref()
                        .is_some_and(|b| b.pushed_at(resume, laid).is_some())
            })
            .map(|p| p.0)
            .fold(start, f32::max);
        self.start = Some(open);
        // Перед первым элементом нет ранней границы этого потока. Нельзя
        // заменить ослабление avoid переносом всей колонки с пустого листа.
        if open > self.floor.unwrap_or(top) + COORD_EPS {
            out.avoid.push((open, next + 0.05));
        }
    }
}
