//! Наблюдаемые границы сборки документа; отсутствуют без feature engine-bench.

use std::time::Duration;

/// Интервалы существующих проходов, без повторного разбора или изменения дерева.
#[derive(Clone, Copy, Debug, Default)]
pub struct BuildTimings {
    /// HTML/XML → исходный DOM и проверка допустимости входа.
    pub parse: Duration,
    /// CSS, каскад, computed-значения и построение нормализованного дерева коробок.
    pub style: Duration,
}

impl BuildTimings {
    /// Отражает features самого движка, включая объединение зависимостей Cargo.
    pub const WPT_HARNESS_ENABLED: bool = cfg!(feature = "wpt-harness");
}

impl super::Document {
    /// Только последний успешный build; unchanged update не запускает проходы.
    pub fn build_timings(&self) -> BuildTimings {
        self.timings
    }
}
