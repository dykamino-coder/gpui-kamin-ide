//! Дорожки рядов сеток из финальной раскладки Taffy.
use super::Watched;
use gpui::{Pixels, Point, Window};
use std::collections::HashMap;
impl Watched {
    /// Row tracks `(start, end)` of the watched grids relative to `origin`,
    /// read from the laid-out grid (taffy `DetailedLayoutInfo::Grid`) through
    /// its first recorded item, before the layout engine is released.
    pub(crate) fn rows(
        &self,
        origin: Point<Pixels>,
        window: &mut Window,
    ) -> HashMap<u64, Vec<(f32, f32)>> {
        let mut grids = Vec::new();
        if let Some(b) = &self.blocks {
            b.grids(&mut grids);
        }
        let mut out = HashMap::new();
        for (grid, items) in grids {
            let Some(lid) = items.iter().find_map(|&id| self.seen(id)) else {
                continue;
            };
            let Some((o, _, rows)) = window.parent_grid_tracks(lid) else {
                continue;
            };
            let off = f32::from(o.y - origin.y);
            out.insert(grid, rows.iter().map(|r| (r.0 + off, r.1 + off)).collect());
        }
        out
    }
}
