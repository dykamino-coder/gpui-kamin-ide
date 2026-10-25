//! Запас копий для рядов после измерения детей; геометрия не оценивается.

use crate::layout::multicol::column_stack::ColumnStack;

impl ColumnStack {
    /// Высоту для числа копий даёт главная разложенная копия, включая распорки роста.
    pub(super) fn reserve_row_copies(&mut self) {
        let Some(rows) = self.rows else { return };
        // TRANSITIONAL(F2-4): Смешанный clone/nested/OOF preplan исключён из роста копий; удалить общий gate в P21 после recorded child resume поверх P05/P08/P10.
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
        // TRANSITIONAL(F2-3): Запас копий рядов ограничен 48 вместо завершения row tokens; удалить предел в P11 после проверки реального прогресса продолжений.
        let copies = (span + self.count + 1).min(48).max(self.copies);
        self.copies = copies;
        for c in self.children.iter_mut().filter(|c| !c.span) {
            let Some(r) = c.regrow.as_ref() else { continue };
            while c.frags.len() + 1 < copies {
                c.fragment_builders.clear();
                c.frags
                    .push((r.build)(&r.dom, c.frags.len() + 1, false, &r.opts));
            }
        }
    }
}

#[cfg(test)]
mod tests;

/// Геометрия полос и их содержимое используют одно дерево после роста.
pub(super) fn rebuild(c: &mut crate::layout::fragment::types::StackChild) {
    let Some(r) = &c.regrow else { return };
    let (el, frags) = r.rebuild(c.frags.len());
    c.fragment_builders.clear();
    c.el = el;
    c.frags = frags;
    if let Some(repeat) = &mut c.repeat {
        for (i, el) in repeat.head_els.iter_mut().enumerate() {
            *el = (r.build)(&r.dom, i + 1, true, &r.opts);
        }
        for (i, el) in repeat.foot_els.iter_mut().enumerate() {
            *el = (r.build)(&r.dom, i, true, &r.opts);
        }
    }
    if let Some(w) = &c.watched {
        w.reset();
    }
}

#[cfg(test)]
mod repeat_tests;
