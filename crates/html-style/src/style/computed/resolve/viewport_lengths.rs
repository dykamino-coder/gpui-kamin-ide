//! Проверка без записи: есть ли у стиля длины в единицах окна или контейнера, которые решает
//! `Computed::resolve_viewport`. Стиль коробки общий (P-18): копия нужна, только когда решать есть что.

use super::unit_change::Probe;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use crate::values::ValueCx;

impl Computed {
    /// Есть ли что решать `resolve_viewport`. Проверка идёт тем же обходом, что и запись
    /// (`decor_inset::walk_decor_inset_lengths`, `unit_lengths::walk_unit_lengths`), только
    /// длины не сворачивает, а отмечает те, что свернул бы `Len::fold_viewport`. «Да» — такая
    /// длина есть или обход пишет в стиль и без неё (не приведённые стопы градиента,
    /// отрицательный радиус).
    pub fn has_viewport_lengths(&self, vcx: ValueCx<'_>) -> bool {
        let seen = std::cell::Cell::new(false);
        let fix = |length: &mut Option<Len>| {
            if length.is_some_and(|l| l.folds_viewport(vcx)) {
                seen.set(true);
            }
        };
        let mut probe = Probe {
            style: self,
            seen: &seen,
            changed: false,
        };
        super::decor_inset::walk_decor_inset_lengths(&mut probe, fix, vcx);
        super::unit_lengths::walk_unit_lengths(&mut probe, fix, vcx);
        probe.changed || seen.get()
    }
}

#[cfg(test)]
#[path = "viewport_lengths_tests.rs"]
mod tests;
