//! Чистый план среза по измеренным строкам и цепочке блочных предков.

mod automatic;
mod boxes;
mod counted;
mod fit;
mod geometry;
mod paragraph;
#[cfg(test)]
mod tests;

use super::ClampEntry;
pub(crate) use boxes::ClampBox;
pub(super) use boxes::{chain_end, chain_extra, chain_sizes};
pub(crate) use geometry::ClampGeometry;

/// Как применить новый срез `v` к записи реестра `prev`.
#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
pub(super) struct CutUpdate {
    /// Записать `v`: реестр всегда хранит точное значение последнего замера,
    /// иначе потребители (`clamp_extent`, высота коробки) читают устаревшую
    /// геометрию.
    pub write: bool,
    /// Запросить ещё кадр. Допуск 4px (сдвиг полей, вызванный самим срезом)
    /// гасит лишние кадры, но не саму запись.
    pub reframe: bool,
}

#[cfg(test)]
pub(super) fn cut_update(prev: Option<f32>, v: f32) -> CutUpdate {
    let reframe = match prev {
        Some(a) if a.is_finite() && v.is_finite() => (a - v).abs() > 4.0,
        Some(a) => a.is_finite() != v.is_finite(),
        None => true,
    };
    CutUpdate {
        write: prev.is_none_or(|a| a.to_bits() != v.to_bits()),
        reframe,
    }
}
