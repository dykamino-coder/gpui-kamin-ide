//! Сетки наблюдаемой копии по финальной раскладке Taffy: дорожки блочной оси, ряды элементов из
//! их размещения и рост после дорожек, который фрагментация передаёт решателю (css-grid-2 §12).
use super::{Watched, axis::BlockAxis};
use gpui::Window;
use std::collections::HashMap;

/// Ряды и размещение элементов сеток копии.
pub(super) struct GridFrames {
    pub(super) rows: HashMap<u64, Vec<(f32, f32)>>,
    pub(super) placements: HashMap<u64, (usize, usize)>,
}

impl Watched {
    /// Дорожки по ID самой разложенной сетки: пустая сетка также имеет
    /// геометрию; ребёнок для чтения `DetailedLayoutInfo::Grid` не нужен.
    /// Ряды элемента — линии его размещения по той же оси, что дорожки.
    pub(super) fn grid_frames(&self, axis: BlockAxis, window: &mut Window) -> GridFrames {
        let mut grids = Vec::new();
        if let Some(b) = &self.blocks {
            b.grids(&mut grids);
        }
        let block = super::axis::solver_axis(axis.axis);
        let mut out = GridFrames {
            rows: HashMap::new(),
            placements: HashMap::new(),
        };
        for (grid, items) in grids {
            let Some(lid) = self.seen.borrow().get(&grid).copied() else {
                continue;
            };
            let Some(normal) = lid.bounds(window) else {
                continue;
            };
            let Some((_, columns, rows)) = window.grid_tracks(lid.id) else {
                continue;
            };
            out.rows
                .insert(grid, axis.tracks(normal.origin, columns, rows, false));
            for item in items {
                let Some(node) = self.seen.borrow().get(&item).and_then(|n| n.id_in(window)) else {
                    continue;
                };
                if let Some(lines) = window.grid_item_lines(node, block) {
                    out.placements.insert(item, tracks_of(lines));
                }
            }
        }
        out
    }

    /// Перенос ряда: решатель вставляет `growth` после дорожки `track` блочной оси сетки `grid`.
    pub(crate) fn grow_grid_row(&self, grid: u64, track: usize, growth: f32) -> bool {
        if !growth.is_finite() || growth <= 0.0 {
            return false;
        }
        let mut all = self.grid_growth.borrow_mut();
        let tracks = all.entry(grid).or_default();
        tracks.resize(tracks.len().max(track + 1), 0.0);
        tracks[track] += growth;
        true
    }
}

/// Полуоткрытые линии размещения с 1 → `(первая, последняя)` дорожки с 0.
fn tracks_of((start, end): (u16, u16)) -> (usize, usize) {
    let first = usize::from(start.max(1)) - 1;
    (first, (usize::from(end.max(2)) - 2).max(first))
}

#[cfg(test)]
mod tests {
    #[test]
    fn placement_lines_map_to_inclusive_track_indices() {
        assert_eq!(super::tracks_of((1, 2)), (0, 0));
        assert_eq!(super::tracks_of((2, 5)), (1, 3));
    }
}
