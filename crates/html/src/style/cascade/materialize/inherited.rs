//! Унаследованные вычисленные данные родителя — ровно то, что читает у родителя стадия вычисленных
//! значений (`compute::compute`) у элемента без каскадного `inherit`: группа наследуемых значений
//! (`InheritedStyle`) и цвета полос прокрутки. Их равенство — условие повторного использования
//! материализованного стиля ребёнка (Blink `ComputedStyle::InheritedEqual`): сперва по указателю
//! на общую группу.

use crate::style::computed::Computed;

/// Совпадают ли у `a` и `b` все поля, которые `compute::compute` берёт у родителя.
pub(super) fn inherited_eq(a: &Computed, b: &Computed) -> bool {
    a.inh == b.inh && a.scroll.colors == b.scroll.colors
}

/// Унаследованные данные `from` — в `onto`: родитель с наследуемыми значениями коробки и
/// ненаследуемыми значениями элемента.
pub(super) fn take_inherited(onto: &mut Computed, from: &Computed) {
    onto.inh = from.inh.clone();
    onto.scroll.colors = from.scroll.colors.clone();
}
