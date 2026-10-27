//! Замороженные дескрипторы метрик; прежнее разрешение равных наклонов без backend.

use super::FontRegistrySnapshot;
use std::collections::HashMap;

pub type MetricFaces = HashMap<String, Vec<(u8, [Option<f32>; 3])>>;

/// Механически сохранён порядок наклонов и отказ при разных значениях одного наклона.
pub fn uniform_slope<T: Copy + PartialEq>(faces: &[(u8, T)], want: u8) -> Option<T> {
    let order = match want {
        1 => [1, 2, 0],
        2 => [2, 0, 1],
        _ => [0, 2, 1],
    };
    for slope in order {
        let mut hit = faces.iter().filter(|(k, _)| *k == slope).map(|(_, v)| *v);
        if let Some(first) = hit.next() {
            return hit.all(|v| v == first).then_some(first);
        }
    }
    None
}

impl FontRegistrySnapshot {
    pub fn size_adjust(&self, family: &str, slope: u8) -> f32 {
        self.registry
            .size_adjust
            .get(&family.to_ascii_lowercase())
            .and_then(|faces| uniform_slope(faces, slope))
            .unwrap_or(1.0)
    }

    pub fn metric_overrides(&self, family: &str, slope: u8) -> [Option<f32>; 3] {
        self.registry
            .metric_overrides
            .get(&family.to_ascii_lowercase())
            .and_then(|faces| uniform_slope(faces, slope))
            .unwrap_or([None; 3])
    }

    /// Прежний нейтральный транспорт дескрипторов в списке тегов адаптера.
    pub fn metric_features(&self, family: &str, slope: u8) -> Vec<(String, u32)> {
        ["kasc", "kdsc", "kgap"]
            .into_iter()
            .zip(self.metric_overrides(family, slope))
            .filter_map(|(tag, v)| v.map(|v| (tag.to_string(), v.to_bits())))
            .collect()
    }
}
