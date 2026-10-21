//! Анимации и переходы: от разрешённого стиля к GPUI-элементам, меняющимся во времени.
//!
//! Кадры `@keyframes` разрешаются ещё при обходе DOM (`dom::walk`); здесь они
//! интерполируются (`frames`, `easing`), накладываются на узел
//! (`animation_frame`, `animation_live`) и оборачиваются в переход
//! (`transition`). `motion` — offset-трансформ по `offset-path`, отдельный
//! проход по собранному дереву.

mod animation_frame;
pub(super) mod animation_live;
pub(crate) mod easing;
pub mod frames;
pub(crate) mod motion;
mod transition;
