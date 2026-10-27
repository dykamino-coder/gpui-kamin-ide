//! Многоколоночная раскладка (css-multicol-1): контейнер и его дети, стопка колонок с
//! балансировкой и разбиением фрагментов, текстовый поток колонок, спаннеры, правила
//! зазоров, мера коробки до сборки (`measure`) и экспорт базовых линий содержимого по тем
//! же фрагментам, что и отрисовка.

mod column_baselines;
pub mod column_flow;
mod column_measure;
mod column_regrow;
pub mod column_stack;
pub mod container;
mod deferred;
pub(crate) mod dimensions;
pub mod gap_rules;
pub mod measure;
pub mod spanner;
mod stack_child;
