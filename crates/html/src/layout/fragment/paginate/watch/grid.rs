//! Дорожки рядов сеток из финальной раскладки Taffy.
use super::{Watched, axis::BlockAxis};
use gpui::Window;
use std::collections::HashMap;
impl Watched {
    /// Дорожки по ID самой разложенной сетки: пустая сетка также имеет
    /// геометрию; ребёнок для чтения `DetailedLayoutInfo::Grid` не нужен.
    pub(super) fn rows(
        &self,
        axis: BlockAxis,
        window: &mut Window,
    ) -> HashMap<u64, Vec<(f32, f32)>> {
        let mut grids = Vec::new();
        if let Some(b) = &self.blocks {
            b.grids(&mut grids);
        }
        let mut out = HashMap::new();
        for (grid, _) in grids {
            let Some(lid) = self.seen(grid) else {
                continue;
            };
            let Some((o, columns, rows)) = window.grid_tracks(lid) else {
                continue;
            };
            out.insert(grid, axis.tracks(o, columns, rows, false));
        }
        out
    }
}
