//! Разложенная геометрия копии, которую читают точки разрыва блочной стопки: border box
//! наблюдаемых коробок по номеру узла и кадры таблиц — коробка таблицы, коробка её сетки и
//! дорожки рядов сетки (css-tables-3 §fragmentation: ряды — единицы разрыва таблицы), и
//! дорожки рядов сеток (css-grid-1 §12, taffy `DetailedLayoutInfo::Grid`). Всё —
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

/// Разложенная копия: коробки по номеру узла, таблицы по ключу (`table_key`)
/// и дорожки рядов сеток по номеру контейнера.
#[derive(Clone, Debug, Default)]
pub(crate) struct Laid {
    pub(crate) boxes: HashMap<u64, (f32, f32)>,
    pub(crate) tables: HashMap<u64, TableFrame>,
    pub(crate) grids: HashMap<u64, Vec<(f32, f32)>>,
    /// Ряды элементов сеток из финального размещения решателя: `(первый, последний)` по индексам
    /// дорожек `grids` (css-grid-2 §8: область задают линии размещения, а не коробка элемента).
    pub(crate) placements: HashMap<u64, (usize, usize)>,
    /// Место, которое фрагментация вставила после дорожек сеток (`Watched::grow_grid_row`):
    /// перенос ряда, а не промежуток `row-gap` между дорожками.
    pub(crate) row_growth: HashMap<u64, Vec<f32>>,
    pub(crate) flexes: HashMap<u64, super::FlexFrame>,
    pub(crate) content: HashMap<u64, Vec<super::super::ContentLine>>,
    pub(crate) descendants: HashMap<u64, Vec<u64>>,
    pub(crate) lines: HashMap<u64, Vec<super::super::LinePush>>,
    pub(crate) flows: HashMap<u64, super::super::FlowGeometry>,
    /// Разрешённые физические поля flex-элементов `(верх, низ)`: граница между
    /// элементами колонки — промежуток между их margin box (css-flexbox-1 §10).
    pub(crate) margins: HashMap<u64, (f32, f32)>,
}

impl Laid {
    /// Границы детей BandFlow измерены до освобождения временных проб.
    /// Они имеют окончательные смещения слотов; наблюдаемый узел ребёнка
    /// ещё может хранить локальные координаты последней пробы.
    pub(crate) fn with_flow(mut self, flow: &super::super::FlowGeometry) -> Self {
        for (&id, &bounds) in &flow.boxes {
            self.boxes.insert(id, bounds);
        }
        self
    }

    /// Border box коробки `id`.
    pub(crate) fn at(&self, id: u64) -> Option<(f32, f32)> {
        self.boxes.get(&id).copied()
    }

    /// Margin box коробки `id` по блочной оси; без записанных полей — border box.
    pub(crate) fn margin_box(&self, id: u64) -> Option<(f32, f32)> {
        let (top, bottom) = self.at(id)?;
        let (mt, mb) = self.margins.get(&id).copied().unwrap_or_default();
        Some((top - mt, bottom + mb))
    }

    /// Дорожки рядов сетки `grid`.
    pub(crate) fn rows(&self, grid: u64) -> Option<Vec<(f32, f32)>> {
        self.grids.get(&grid).cloned()
    }

    /// Место, вставленное фрагментацией после дорожки `track` сетки `grid`.
    pub(crate) fn growth_after(&self, grid: u64, track: usize) -> f32 {
        self.row_growth
            .get(&grid)
            .and_then(|g| g.get(track).copied())
            .unwrap_or(0.0)
    }

    /// Ряды элемента сетки `id` по его размещению.
    pub(crate) fn item_rows(&self, id: u64) -> Option<(usize, usize)> {
        self.placements.get(&id).copied()
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
            grids: HashMap::new(),
            placements: HashMap::new(),
            row_growth: HashMap::new(),
            flexes: HashMap::new(),
            flows: HashMap::new(),
            lines: HashMap::new(),
            content: HashMap::new(),
            descendants: HashMap::new(),
            margins: HashMap::new(),
        }
    }
}
