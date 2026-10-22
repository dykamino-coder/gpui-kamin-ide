//! Запас копий для рядов после измерения детей; геометрия не оценивается.

use crate::layout::multicol::column_stack::ColumnStack;

impl ColumnStack {
    /// Ряды используют прежний предел копий (ADR P-19 F2-1), но высоту
    /// для их числа даёт главная разложенная копия, включая распорки роста.
    pub(super) fn reserve_row_copies(&mut self) {
        let Some(rows) = self.rows else { return };
        // Общий старый preplan clone/nested/OOF (F2-1/2/3) не смешивается
        // с ещё не измеренной копией: тот же контракт, что у laid::children.
        if self
            .children
            .iter()
            .filter(|c| !c.span)
            .any(|c| c.measure.is_none() || c.regrow.is_none())
        {
            return;
        }
        let per = rows.h.unwrap_or(f32::MAX).max(1.0);
        let span = self
            .children
            .iter()
            .filter(|c| !c.span)
            .map(|c| (c.h.max(c.over) / per).ceil() as usize)
            .max()
            .unwrap_or(0);
        let copies = (span + self.count + 1).min(48).max(self.copies);
        self.copies = copies;
        for c in self.children.iter_mut().filter(|c| !c.span) {
            let Some(r) = c.regrow.as_ref() else { continue };
            while c.frags.len() + 1 < copies {
                c.frags.push((r.build)(&r.dom, c.frags.len() + 1));
            }
        }
    }
}

#[cfg(test)]
mod tests;
