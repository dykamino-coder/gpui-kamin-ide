//! Копии ребёнка у фрагмента плана — какая копия рисует фрагмент и какие
//! копии шапки/подвала таблицы стоят у него и где; общее для стопки колонок
//! и стопки страниц.

use super::{Frag, Repeat};
use crate::tolerance::COORD_EPS;
use gpui::AnyElement;

/// Копия ребёнка для фрагмента с номером копии `copy`: нулевая — сам
/// элемент, остальные — запасные копии (`StackChild::frags`,
/// `PageKid::frags`); `None` — копий не хватило (остаток за кадром).
pub(crate) fn frag_copy<'a>(
    el: &'a mut AnyElement,
    frags: &'a mut [AnyElement],
    copy: usize,
) -> Option<&'a mut AnyElement> {
    match copy {
        0 => Some(el),
        n => frags.get_mut(n - 1),
    }
}

impl Repeat {
    /// Копии полос фрагмента `f` для раскладки: копия и её сдвиг от верха
    /// содержимого фрагмента (`f.y`). Шапка — у продолжения (`f.copy ≥ 1`),
    /// поднята так, что её полоса встаёт над содержимым; подвал — у
    /// непоследнего фрагмента, сразу под содержимым (`Repeat`).
    pub(crate) fn place_bands(&mut self, f: &Frag, mut place: impl FnMut(&mut AnyElement, f32)) {
        if let Some((src, _)) = self.head.filter(|_| f.head > COORD_EPS && f.copy > 0)
            && let Some(el) = self.head_els.get_mut(f.copy - 1)
        {
            place(el, -f.head - src);
        }
        if let Some((src, bh)) = self.foot.filter(|_| f.foot > COORD_EPS)
            && let Some(el) = self.foot_els.get_mut(f.copy)
        {
            place(el, f.h + f.foot - bh - src);
        }
    }

    /// Копии полос фрагмента `f` для отрисовки: копия и видимая полоса
    /// `(верх, высота)` от верха содержимого фрагмента — по ней режет маска.
    pub(crate) fn paint_bands(
        &mut self,
        f: &Frag,
        mut paint: impl FnMut(&mut AnyElement, f32, f32),
    ) {
        if f.head > COORD_EPS
            && f.copy > 0
            && let Some(el) = self.head_els.get_mut(f.copy - 1)
        {
            paint(el, -f.head, f.head);
        }
        if f.foot > COORD_EPS
            && let Some((_, bh)) = self.foot
            && let Some(el) = self.foot_els.get_mut(f.copy)
        {
            paint(el, f.h + f.foot - bh, bh);
        }
    }
}

#[cfg(test)]
mod tests;
