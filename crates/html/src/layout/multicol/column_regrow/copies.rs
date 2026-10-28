//! Запас копий после измерения детей: число копий ребёнка — число фрагментов его
//! измеренной протяжённости в плане; геометрия не оценивается.

use crate::layout::fragment::types::StackChild;
use crate::layout::multicol::column_stack::ColumnStack;

impl ColumnStack {
    /// Высоту для числа копий даёт главная разложенная копия, включая распорки роста.
    pub(super) fn reserve_row_copies(&mut self) {
        if self.rows.is_none() {
            return;
        }
        let old = self.copies;
        self.copies = usize::MAX;
        let (_, _, plan, _) = self.balance(&self.kid_geoms());
        self.copies = old.max(plan.iter().map(|f| f.col + 1).max().unwrap_or(1));
        for (index, c) in self
            .children
            .iter_mut()
            .enumerate()
            .filter(|(_, c)| !c.span)
        {
            let copies = plan
                .iter()
                .filter(|f| f.kid == index)
                .map(|f| f.copy + 1)
                .max()
                .unwrap_or(1);
            reserve(c, copies);
        }
    }

    /// Переполняющие колонки (css-multicol-1 §8.2: при заданной высоте колонок
    /// содержимое, которому не хватило колонок, создаёт новые в строчном
    /// направлении): копий у ребёнка — сколько колонок займёт его измеренный
    /// поток, включая продолжения внепоточных потомков в его копиях.
    pub(crate) fn reserve_overflow_copies(&mut self) {
        let (Some(per), None) = (self.fixed_height.filter(|h| *h > 0.0), self.rows) else {
            return;
        };
        for c in self.children.iter_mut().filter(|c| !c.span) {
            let need = (c.h.max(c.over) / per).ceil() as usize + 1;
            reserve(c, need);
            self.copies = self.copies.max(c.frags.len() + 1);
        }
    }
}

/// Достроить копии ребёнка до `copies`: выросшее дерево (`Regrow`) или его рецепт.
fn reserve(c: &mut StackChild, copies: usize) {
    while c.frags.len() + 1 < copies {
        let part = c.frags.len() + 1;
        let el = if let Some(r) = c.regrow.as_ref() {
            (r.build)(&r.dom, part, false, &r.opts)
        } else if let Some(copier) = c.copier.as_ref() {
            copier(part)
        } else {
            return;
        };
        c.fragment_builders.clear();
        c.frags.push(el);
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
