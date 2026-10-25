//! Унаследованные вычисленные данные родителя — ровно то, что читает у родителя стадия вычисленных
//! значений (`compute::compute`) у элемента без каскадного `inherit`: группа наследуемых значений
//! (`InheritedStyle`) и цвета полос прокрутки. Их равенство — условие повторного использования
//! материализованного стиля ребёнка (Blink `ComputedStyle::InheritedEqual`): сперва по указателю
//! на общую группу.

use crate::style::computed::Computed;

/// Совпадают ли у `a` и `b` все поля, которые `compute::compute` берёт у родителя.
pub(super) fn inherited_eq(a: &Computed, b: &Computed) -> bool {
    a.inh == b.inh
        && a.scroll.colors == b.scroll.colors
        && a.container_box_for_children() == b.container_box_for_children()
}
