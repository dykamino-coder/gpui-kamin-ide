//! Проверка без записи: есть ли у стиля длины в единицах окна или контейнера, которые решает
//! `Computed::resolve_viewport`. Стиль коробки общий (P-18): копия нужна, только когда решать есть что.

use super::unit_change::Probe;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use crate::values::ValueCx;

impl Computed {
    /// Есть ли что решать `resolve_viewport`. Проверка идёт тем же обходом, что и запись
    /// (`decor_inset::walk_decor_inset_lengths`, `unit_lengths::walk_unit_lengths`), только
    /// шаги не пишут, а отвечают по ссылке — без копий и выделений памяти. «Да» — есть длина,
    /// которую свернул бы `Len::fold_viewport`, или обход пишет в стиль и без неё (не
    /// приведённые стопы градиента, отрицательный радиус).
    pub fn has_viewport_lengths(&self, vcx: ValueCx<'_>) -> bool {
        let moves = |length: Len| length.folds_viewport(vcx);
        let fix = |_: &mut Option<Len>| {};
        let mut probe = Probe {
            style: self,
            pending: false,
        };
        super::decor_inset::walk_decor_inset_lengths(&mut probe, moves, fix, vcx);
        super::unit_lengths::walk_unit_lengths(&mut probe, moves, fix, vcx);
        probe.pending
    }
}

#[cfg(test)]
#[path = "viewport_lengths_tests.rs"]
mod tests;
