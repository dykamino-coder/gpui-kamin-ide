//! Разложенная геометрия копии, которую читают точки разрыва блочной стопки: border box
//! наблюдаемых коробок по номеру узла и кадры таблиц — коробка таблицы, коробка её сетки и
//! дорожки рядов сетки (css-tables-3 §fragmentation: ряды — единицы разрыва таблицы). Всё —
//! `(верх, низ)` в координатах потока копии.

use std::collections::HashMap;

/// Разложенная таблица: коробка таблицы (без подписей), коробка сетки рядов
/// (содержимое коробки таблицы) и дорожки рядов в порядке `grid_rows`.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct TableFrame {
    pub(crate) table: (f32, f32),
    pub(crate) grid: (f32, f32),
    pub(crate) rows: Vec<(f32, f32)>,
}

/// Разложенная копия: коробки по номеру узла и таблицы по ключу (`table_key`).
#[derive(Clone, Debug, Default)]
pub(crate) struct Laid {
    pub(crate) boxes: HashMap<u64, (f32, f32)>,
    pub(crate) tables: HashMap<u64, TableFrame>,
}

impl Laid {
    /// Border box коробки `id`.
    pub(crate) fn at(&self, id: u64) -> Option<(f32, f32)> {
        self.boxes.get(&id).copied()
    }

    /// Кадр таблицы с ключом `key`.
    pub(crate) fn table(&self, key: u64) -> Option<&TableFrame> {
        self.tables.get(&key)
    }

    /// Разложенная копия из одних коробок.
    #[cfg(test)]
    pub(crate) fn of_boxes(boxes: &[(u64, f32, f32)]) -> Self {
        Self {
            boxes: boxes.iter().map(|&(id, t, b)| (id, (t, b))).collect(),
            tables: HashMap::new(),
        }
    }
}
