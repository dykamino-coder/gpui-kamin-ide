//! Per-buffer damage on the scene comparator's integer-pixel tile grid.
use super::super::Region;
use gpui::{DAMAGE_TILE, SceneDamage};
use std::collections::HashMap;

pub(super) struct DamageTiles {
    size: (u32, u32),
    changed: Vec<bool>,
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
            false,
        );
        self.changed.fill(false);
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
                for &r in regions {
                    self.mark(r);
                }
            }
        }
    }

    fn mark(&mut self, [x, y, width, height]: [u32; 4]) {
        let cols = self.size.0.div_ceil(DAMAGE_TILE) as usize;
        let right = x
            .saturating_add(width)
            .min(self.size.0)
            .div_ceil(DAMAGE_TILE);
        let bottom = y
            .saturating_add(height)
            .min(self.size.1)
            .div_ceil(DAMAGE_TILE);
        if width == 0 || height == 0 {
            return;
        }
        for row in y / DAMAGE_TILE..bottom {
            for col in x / DAMAGE_TILE..right {
                self.changed[row as usize * cols + col as usize] = true;
            }
        }
    }

    /// Return disjoint, vertically coalesced runs; None requires a whole frame.
    pub(super) fn regions(&self) -> Option<Vec<Region>> {
        if self.unknown {
            return None;
        }
        let (w, h) = self.size;
        let cols = w.div_ceil(DAMAGE_TILE) as usize;
        let mut out: Vec<Region> = Vec::new();
        let mut previous: HashMap<(u32, u32), usize> = HashMap::new();
        for row in 0..h.div_ceil(DAMAGE_TILE) {
            let mut current = HashMap::new();
            let mut col = 0;
            while col < cols {
                if !self.changed[row as usize * cols + col] {
                    col += 1;
                    continue;
                }
                let first = col;
                while col < cols && self.changed[row as usize * cols + col] {
                    col += 1;
                }
                let x = first as u32 * DAMAGE_TILE;
                let width = (col as u32 * DAMAGE_TILE).min(w) - x;
                let y = row * DAMAGE_TILE;
                let height = DAMAGE_TILE.min(h - y);
                let key = (x, width);
                let index = match previous.get(&key).copied() {
                    Some(i) => {
                        out[i].height += height;
                        i
                    }
                    None => {
                        out.push(Region {
                            x,
                            y,
                            width,
                            height,
                        });
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
mod tests {
    use super::*;

    fn rect(x: u32, y: u32, width: u32, height: u32) -> SceneDamage {
        SceneDamage::Rect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn distant_edits_preserve_clean_tiles_and_clip_the_last_row() {
        let mut tiles = DamageTiles::default();
        tiles.clear((256, 193));
        tiles.merge(rect(1, 1, 1, 1), &[], (256, 193));
        tiles.merge(rect(255, 192, 1, 1), &[], (256, 193));
        let r = tiles.regions().unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!(r.iter().map(|r| r.width * r.height).sum::<u32>(), 4096 + 64);
        assert_eq!(
            r[1],
            Region {
                x: 192,
                y: 192,
                width: 64,
                height: 1
            }
        );
    }

    #[test]
    fn history_unions_overlap_once_and_coalesces_vertical_runs() {
        let mut tiles = DamageTiles::default();
        tiles.clear((256, 192));
        tiles.merge(rect(65, 1, 1, 1), &[], (256, 192));
        tiles.merge(rect(65, 1, 1, 127), &[], (256, 192));
        assert_eq!(
            tiles.regions().unwrap(),
            vec![Region {
                x: 64,
                y: 0,
                width: 64,
                height: 128
            }]
        );
        tiles.merge(SceneDamage::Unknown, &[], (256, 192));
        tiles.merge(SceneDamage::Unchanged, &[], (256, 192));
        assert!(tiles.regions().is_none());
        tiles.clear((256, 192));
        assert!(tiles.regions().unwrap().is_empty());
    }

    #[test]
    fn sparse_regions_override_the_conservative_envelope() {
        let mut tiles = DamageTiles::default();
        tiles.clear((256, 192));
        tiles.merge(
            rect(0, 0, 256, 192),
            &[[0, 0, 64, 64], [192, 128, 64, 64]],
            (256, 192),
        );
        assert_eq!(
            tiles
                .regions()
                .unwrap()
                .iter()
                .map(|r| r.width * r.height)
                .sum::<u32>(),
            8192
        );
        tiles.merge(SceneDamage::Unchanged, &[], (257, 192));
        assert!(tiles.regions().is_none());
    }

    #[test]
    fn every_small_grid_mask_is_covered_exactly_once() {
        let size = (129, 65);
        for mask in 0u32..64 {
            let mut tiles = DamageTiles::default();
            tiles.clear(size);
            for cell in 0..6 {
                if mask & (1 << cell) != 0 {
                    tiles.merge(rect((cell % 3) * 64, (cell / 3) * 64, 1, 1), &[], size);
                }
            }
            let regions = tiles.regions().unwrap();
            for y in 0..size.1 {
                for x in 0..size.0 {
                    let covers = regions
                        .iter()
                        .filter(|r| x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height)
                        .count();
                    let selected = mask & (1 << (x / 64 + (y / 64) * 3)) != 0;
                    assert_eq!(covers, usize::from(selected), "mask={mask} at ({x},{y})");
                }
            }
        }
    }
}
