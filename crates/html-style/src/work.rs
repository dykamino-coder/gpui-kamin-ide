//! Единый словарь наблюдаемой работы движка; без feature аргументы hooks не вычисляются.

#[cfg(feature = "work-counters")]
#[path = "work/enabled.rs"]
mod enabled;
#[cfg(feature = "work-counters")]
pub use enabled::*;
#[cfg(feature = "work-counters")]
pub mod style_storage;

/// Без наблюдателя границ фаз окна совпадают со стенным временем.
#[cfg(not(feature = "work-counters"))]
pub fn observed() -> std::time::Duration {
    std::time::Duration::ZERO
}

/// Момент окна замера фазы. Разность двух моментов — стенное время без того,
/// что поток провёл в наблюдателе границ фаз ([`observed`]): снимок
/// процесса на границе принадлежит замеру, а не фазе, и его стоимость
/// (запросы памяти процесса и GPU) не должна входить ни в одно окно.
/// Без наблюдателя разность равна стенной без изменений.
#[derive(Clone, Copy, Debug)]
pub struct Clock {
    wall: std::time::Instant,
    observed: std::time::Duration,
}
impl Clock {
    pub fn now() -> Self {
        Self {
            wall: std::time::Instant::now(),
            observed: observed(),
        }
    }
    /// Время окна от `earlier` до этого момента без работы наблюдателя.
    pub fn duration_since(self, earlier: Self) -> std::time::Duration {
        let observer = self.observed.saturating_sub(earlier.observed);
        self.wall
            .duration_since(earlier.wall)
            .saturating_sub(observer)
    }
    pub fn elapsed(self) -> std::time::Duration {
        Self::now().duration_since(self)
    }
}

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

    #[test]
    fn windows_without_work_counters_are_wall_time() {
        let before = crate::work::Clock::now();
        let after = crate::work::Clock::now();
        assert_eq!(crate::work::observed(), std::time::Duration::ZERO);
        assert_eq!(
            after.duration_since(before),
            after.wall.duration_since(before.wall)
        );
    }
}
