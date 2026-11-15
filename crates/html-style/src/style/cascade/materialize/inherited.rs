//! Унаследованные вычисленные данные родителя — ровно то, что читает у родителя стадия вычисленных
//! значений (`compute::compute`) у элемента без каскадного `inherit`: группа наследуемых значений
//! (`InheritedStyle`) и цвета полос прокрутки. Их равенство — условие повторного использования
//! материализованного стиля ребёнка (Blink `ComputedStyle::InheritedEqual`): сперва по указателю
//! на общую группу.

use crate::style::computed::Computed;

/// Общий контракт слота и кэша прохода: явное наследование reset-значений
/// требует равенства всего родителя, обычное — только наследуемых входов.
pub(super) fn parent_eq(a: &Computed, b: &Computed, own: &Computed) -> bool {
    std::ptr::eq(a, b)
        || a.shares_storage(b)
        || if inherits_reset(own) {
            a == b
        } else {
            inherited_eq(a, b)
        }
}

/// Whether the computed stage of `own` reads non-inherited values of its
/// parent (css-cascade-5 §7.3.1).
pub(crate) fn inherits_reset(own: &Computed) -> bool {
    let cascade = &own.reset_cascade;
    !cascade.explicit_inherit.is_empty()
        || !cascade.inherit_longhands.is_empty()
        || cascade.inherit_bits != 0
        || cascade.bidi_inherit
        || own.reset_text_box.text_overflow_inherit
        || own.reset_effects.shadow_inherit
        || own.reset_box_layout.scroll.inherit_width
        || own.reset_box_layout.scroll.inherit_gutter
        || own.reset_box_layout.snap.inherit != 0
        || own.reset_background.background_blend == Some(crate::style::computed::BlendMode::Inherit)
}

/// Совпадают ли у `a` и `b` все поля, которые `compute::compute` берёт у родителя.
pub(super) fn inherited_eq(a: &Computed, b: &Computed) -> bool {
    a.inh == b.inh
        && a.reset_box_layout.scroll.colors == b.reset_box_layout.scroll.colors
        && a.container_box_for_children() == b.container_box_for_children()
}
