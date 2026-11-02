//! Единый словарь наблюдаемой работы движка; без feature аргументы hooks не вычисляются.

#[cfg(feature = "work-counters")]
#[path = "work/enabled.rs"]
mod enabled;
#[cfg(feature = "work-counters")]
pub use enabled::*;
#[cfg(feature = "work-counters")]
pub mod style_storage;

/// Наблюдать работу без вычисления аргументов в обычной сборке.
#[cfg(feature = "work-counters")]
#[macro_export]
macro_rules! count_work {
    ($counter:ident, $amount:expr) => {
        $crate::work::increment($crate::work::Counter::$counter, $amount as u64)
    };
}
#[cfg(not(feature = "work-counters"))]
#[macro_export]
macro_rules! count_work {
    ($counter:ident, $amount:expr) => {};
}
pub use crate::count_work as add;

/// Измерить включённый диагностический участок; вложенные интервалы пересекаются.
#[cfg(feature = "work-counters")]
#[macro_export]
macro_rules! time_work {
    ($guard:ident, $counter:ident) => {
        let $guard = $crate::work::time($crate::work::Counter::$counter);
    };
}
#[cfg(not(feature = "work-counters"))]
#[macro_export]
macro_rules! time_work {
    ($guard:ident, $counter:ident) => {};
}
pub use crate::time_work as time;

/// Привязать вложенную работу к фазе, восстановив внешнюю при выходе.
#[cfg(feature = "work-counters")]
#[macro_export]
macro_rules! work_phase {
    ($guard:ident, $phase:ident) => {
        let $guard = $crate::work::enter($crate::work::Phase::$phase);
    };
}
#[cfg(not(feature = "work-counters"))]
#[macro_export]
macro_rules! work_phase {
    ($guard:ident, $phase:ident) => {};
}
pub use crate::work_phase as phase;

/// Граница callbacks, между которыми GPUI самостоятельно выполняет layout.
#[cfg(feature = "work-counters")]
#[macro_export]
macro_rules! work_switch {
    ($phase:ident) => {
        $crate::work::set_phase($crate::work::Phase::$phase)
    };
}
#[cfg(not(feature = "work-counters"))]
#[macro_export]
macro_rules! work_switch {
    ($phase:ident) => {};
}
pub use crate::work_switch as switch;

/// Исход уже выполненного поиска; hook не повторяет сам поиск.
#[cfg(feature = "work-counters")]
#[macro_export]
macro_rules! work_cache {
    ($value:expr, $hit:ident, $miss:ident) => {
        if $value.is_some() {
            $crate::work::add!($hit, 1);
        } else {
            $crate::work::add!($miss, 1);
        }
    };
}
#[cfg(not(feature = "work-counters"))]
#[macro_export]
macro_rules! work_cache {
    ($value:expr, $hit:ident, $miss:ident) => {};
}
pub use crate::work_cache as cache;

#[cfg(all(test, not(feature = "work-counters")))]
mod tests {
    #[test]
    fn disabled_hook_does_not_evaluate_or_resolve_arguments() {
        crate::work::add!(NotEvenACounter, panic!("must disappear"));
        crate::work::phase!(_phase, NotEvenAPhase);
        crate::work::switch!(NotEvenAPhase);
        crate::work::cache!(unresolved_value, NotAHit, NotAMiss);
        crate::work::time!(_timer, NotEvenACounter);
    }
}
