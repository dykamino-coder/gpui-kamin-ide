//! Полосы повтора таблицы во фрагменте: какая копия шапки/подвала стоит у
//! фрагмента плана и где — общее для стопки колонок и стопки страниц.

use super::{Frag, Repeat};
use crate::tolerance::COORD_EPS;
use gpui::AnyElement;

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
