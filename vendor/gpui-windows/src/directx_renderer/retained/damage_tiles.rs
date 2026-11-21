//! Per-buffer damage with tight pixel bounds inside the scene's tile grid.
use super::super::Region;
use gpui::{DAMAGE_TILE, SceneDamage};
use std::collections::HashMap;

pub(super) struct DamageTiles {
    size: (u32, u32),
    changed: Vec<Option<Region>>,
    unknown: bool,
}

impl Default for DamageTiles {
    fn default() -> Self {
        Self {
            size: (0, 0),
            changed: Vec::new(),
            unknown: true,
        }
    }
}

impl DamageTiles {
    // regions() emits disjoint rectangles clipped to the target.
    pub(super) fn covers_target(regions: &[Region], size: (u32, u32)) -> bool {
        let area: u64 = regions
            .iter()
            .map(|r| u64::from(r.width) * u64::from(r.height))
            .sum();
        area == u64::from(size.0) * u64::from(size.1)
    }

    pub(super) fn clear(&mut self, size: (u32, u32)) {
        self.size = size;
        self.changed.resize(
            (size.0.div_ceil(DAMAGE_TILE) as usize) * (size.1.div_ceil(DAMAGE_TILE) as usize),
            None,
        );
        self.changed.fill(None);
        self.unknown = false;
    }

    pub(super) fn merge(&mut self, damage: SceneDamage, regions: &[[u32; 4]], size: (u32, u32)) {
        if self.unknown {
            return;
        }
        if size != self.size || damage == SceneDamage::Unknown {
            self.unknown = true;
            return;
        }
        if let SceneDamage::Rect {
            x,
            y,
            width,
            height,
        } = damage
        {
            if regions.is_empty() {
                self.mark([x, y, width, height]);
            } else {
                for &region in regions {
                    self.mark(region);
                }
            }
        }
    }

    fn mark(&mut self, [x, y, width, height]: [u32; 4]) {
        let right = x.saturating_add(width).min(self.size.0);
        let bottom = y.saturating_add(height).min(self.size.1);
        if x >= right || y >= bottom {
            return;
        }
        let cols = self.size.0.div_ceil(DAMAGE_TILE) as usize;
        for row in y / DAMAGE_TILE..bottom.div_ceil(DAMAGE_TILE) {
            for col in x / DAMAGE_TILE..right.div_ceil(DAMAGE_TILE) {
                let left = x.max(col * DAMAGE_TILE);
                let top = y.max(row * DAMAGE_TILE);
                let end_x = right.min(col.saturating_add(1).saturating_mul(DAMAGE_TILE));
                let end_y = bottom.min(row.saturating_add(1).saturating_mul(DAMAGE_TILE));
                let cell = &mut self.changed[row as usize * cols + col as usize];
                *cell = Some(match *cell {
                    None => Region {
                        x: left,
                        y: top,
                        width: end_x - left,
                        height: end_y - top,
                    },
                    Some(old) => {
                        let x = old.x.min(left);
                        let y = old.y.min(top);
                        Region {
                            x,
                            y,
                            width: (old.x + old.width).max(end_x) - x,
                            height: (old.y + old.height).max(end_y) - y,
                        }
                    }
                });
            }
        }
    }

    /// Return disjoint pixel rectangles; unknown buffer contents require a whole frame.
    /// History may fill a gap inside one cell, never the gap between distant cells.
    pub(super) fn regions(&self) -> Option<Vec<Region>> {
        if self.unknown {
            return None;
        }
        let cols = self.size.0.div_ceil(DAMAGE_TILE) as usize;
        let mut out: Vec<Region> = Vec::new();
        let mut previous: HashMap<(u32, u32), usize> = HashMap::new();
        for cells in self.changed.chunks(cols.max(1)) {
            let mut runs: Vec<Region> = Vec::new();
            for &region in cells.iter().flatten() {
                if let Some(last) = runs.last_mut()
                    && last.x + last.width == region.x
                    && last.y == region.y
                    && last.height == region.height
                {
                    last.width += region.width;
                } else {
                    runs.push(region);
                }
            }
            let mut current = HashMap::new();
            for region in runs {
                let key = (region.x, region.width);
                let index = match previous.get(&key).copied() {
                    Some(i) if out[i].y + out[i].height == region.y => {
                        out[i].height += region.height;
                        i
                    }
                    _ => {
                        out.push(region);
                        out.len() - 1
                    }
                };
                current.insert(key, index);
            }
            previous = current;
        }
        Some(out)
    }
}

#[cfg(test)]
#[path = "damage_tiles/tests.rs"]
mod tests;
