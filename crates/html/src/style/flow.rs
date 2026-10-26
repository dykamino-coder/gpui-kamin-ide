//! Классификация участия в потоке по CSS 2.1 §9.3.1 и §9.5.
//!
//! Предикаты свойств доступны DOM и раскладке без зависимости DOM от layout.

use crate::style::computed::{Computed, Position};

/// Абсолютное позиционирование: absolute/fixed; float сюда не входит.
pub(crate) fn absolutely_positioned(position: Option<Position>) -> bool {
    matches!(position, Some(Position::Absolute | Position::Fixed))
}

/// Вне потока по нормализованному стилю: ненулевой float или absolute/fixed.
pub(crate) fn out_of_flow(style: &Computed) -> bool {
    style.float.is_some_and(|side| side != 0) || absolutely_positioned(style.position)
}

/// До нормализации DOM важен сам факт записи float, включая Some(0).
/// Сохраняет контракт блокификации и прозрачности соседа для run-in.
pub(crate) fn has_float_or_absolute_position(style: &Computed) -> bool {
    style.float.is_some() || absolutely_positioned(style.position)
}
