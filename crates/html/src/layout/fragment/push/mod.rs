//! Перенос коробок на следующий фрагмент распорками роста по РАЗЛОЖЕННОЙ копии
//! (`measured`): точку разрыва и коробку под ней даёт записанная раскладка.
//! Подмодуль `spacer` — сама распорка и монолит `break-inside: avoid`.

mod cell;
mod spacer;
pub(crate) use spacer::avoid_only_monolith;
pub(crate) mod flex_flow;
mod float_flow;
mod measured;
pub(crate) mod monolith_flow;
mod vertical;
pub(crate) use measured::{Growth, Regrow, measured_push, measured_tree, recorded_tree};
