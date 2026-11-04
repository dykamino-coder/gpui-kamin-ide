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
    /// Размещает полосы по очереди, сохраняя общий стек дочерних фаз.
    pub(crate) async fn place_bands_async(
        &mut self,
        f: &Frag,
        mut place: impl AsyncFnMut(&mut AnyElement, f32),
    ) {
        if let Some((src, _)) = self.head.filter(|_| f.head > COORD_EPS && f.copy > 0)
            && let Some(el) = self.head_els.get_mut(f.copy - 1)
        {
            place(el, -f.head - src).await;
        }
        if let Some(top) = self.foot_top(f)
            && let Some((src, _)) = self.foot
            && let Some(el) = self.foot_els.get_mut(f.copy)
        {
            place(el, top - src).await;
        }
    }

    pub(crate) async fn paint_bands_async(
        &mut self,
        f: &Frag,
        mut paint: impl AsyncFnMut(&mut AnyElement, f32, f32),
    ) {
        if f.head > COORD_EPS
            && f.copy > 0
            && let Some(el) = self.head_els.get_mut(f.copy - 1)
        {
            paint(el, -f.head, f.head).await;
        }
        if let Some(top) = self.foot_top(f)
            && let Some((_, bh)) = self.foot
            && let Some(el) = self.foot_els.get_mut(f.copy)
        {
            paint(el, top, bh).await;
        }
    }
}

#[cfg(test)]
mod tests;
