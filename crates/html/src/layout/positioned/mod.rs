//! Позиционированные коробки (css-position-3): содержащий блок, статическая позиция,
//! относительный сдвиг, якоря, предикаты внепоточности, безопасное выравнивание абсолюта
//! в flex-контейнере и повтор позиционированных детей во фрагментах.

pub(crate) mod absolute_overflow;
mod absolute_overflow_math;
pub(crate) mod anchor;
pub mod containing_block;
pub(crate) mod layer_scope;
pub mod predicates;
pub mod relative;
mod spot_geometry;
pub mod static_position;

pub(crate) mod repeat;
