//! Сетка, которую фрагментация спускает стопкой рядов (`grid_stack`); остальные сетки
//! фрагментирует разложенная копия (`flow::blocks::grid`, css-grid-2 §Fragmenting Grid Layout).

mod stack;
pub(crate) use stack::grid_stack;
