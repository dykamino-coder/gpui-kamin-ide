//! Index occupied damage cells once, then query each group in constant time.
use super::super::Region;
use gpui::DAMAGE_TILE;

pub(super) struct GroupDamage {
    cols: usize,
    rows: usize,
    sum: Vec<u32>,
}

impl GroupDamage {
    pub(super) fn new(regions: &[Region]) -> Self {
        let cols = regions
            .iter()
            .map(|r| (r.x + r.width).div_ceil(DAMAGE_TILE))
            .max()
            .unwrap_or(0) as usize;
        let rows = regions
            .iter()
            .map(|r| (r.y + r.height).div_ceil(DAMAGE_TILE))
            .max()
            .unwrap_or(0) as usize;
        let stride = cols + 1;
        let mut diff = vec![0i32; stride * (rows + 1)];
        for r in regions {
            let (x0, y0) = ((r.x / DAMAGE_TILE) as usize, (r.y / DAMAGE_TILE) as usize);
            let (x1, y1) = (
                ((r.x + r.width).div_ceil(DAMAGE_TILE)) as usize,
                ((r.y + r.height).div_ceil(DAMAGE_TILE)) as usize,
            );
            diff[y0 * stride + x0] += 1;
            diff[y0 * stride + x1] -= 1;
            diff[y1 * stride + x0] -= 1;
            diff[y1 * stride + x1] += 1;
        }
        let mut sum = vec![0u32; stride * (rows + 1)];
        for y in 0..rows {
            let mut occupied = 0;
            for x in 0..cols {
                let i = y * stride + x;
                let left = if x > 0 { diff[i - 1] } else { 0 };
                let above = if y > 0 { diff[i - stride] } else { 0 };
                let diagonal = if x > 0 && y > 0 {
                    diff[i - stride - 1]
                } else {
                    0
                };
                diff[i] += left + above - diagonal;
                occupied += u32::from(diff[i] > 0);
                sum[(y + 1) * stride + x + 1] = sum[y * stride + x + 1] + occupied;
            }
        }
        Self { cols, rows, sum }
    }

    /// Clean cells are excluded exactly; occupied-cell intersections are conservative.
    pub(super) fn intersects(&self, [left, top, right, bottom]: [f32; 4]) -> bool {
        if ![left, top, right, bottom].into_iter().all(f32::is_finite) {
            return true;
        }
        if left >= right || top >= bottom {
            return false;
        }
        let (x0, y0) = (
            (left.max(0.) / DAMAGE_TILE as f32).floor() as usize,
            (top.max(0.) / DAMAGE_TILE as f32).floor() as usize,
        );
        let (x1, y1) = (
            (right.max(0.) / DAMAGE_TILE as f32).ceil() as usize,
            (bottom.max(0.) / DAMAGE_TILE as f32).ceil() as usize,
        );
        let (x0, y0, x1, y1) = (
            x0.min(self.cols),
            y0.min(self.rows),
            x1.min(self.cols),
            y1.min(self.rows),
        );
        if x0 >= x1 || y0 >= y1 {
            return false;
        }
        let stride = self.cols + 1;
        let at = |x, y| self.sum[y * stride + x];
        at(x1, y1) + at(x0, y0) > at(x0, y1) + at(x1, y0)
    }
}

#[cfg(test)]
mod tests {
    use super::{DAMAGE_TILE, GroupDamage, Region};

    #[test]
    fn every_small_grid_query_matches_occupied_cells() {
        let tile = DAMAGE_TILE;
        for bits in 0..1u32 << 9 {
            let regions: Vec<_> = (0..9)
                .filter(|i| bits & (1 << i) != 0)
                .map(|i| Region {
                    x: i % 3 * tile + 7,
                    y: i / 3 * tile + 9,
                    width: 3,
                    height: 5,
                })
                .collect();
            let index = GroupDamage::new(&regions);
            for y0 in 0..3 {
                for x0 in 0..3 {
                    for y1 in y0 + 1..=3 {
                        for x1 in x0 + 1..=3 {
                            let expected =
                                (y0..y1).any(|y| (x0..x1).any(|x| bits & (1 << (y * 3 + x)) != 0));
                            let query = [x0, y0, x1, y1].map(|n| (n * tile) as f32);
                            assert_eq!(index.intersects(query), expected, "{bits:09b} {query:?}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn overlapping_regions_empty_and_unknown_queries_are_conservative() {
        let t = DAMAGE_TILE;
        let index = GroupDamage::new(&[
            Region {
                x: 7,
                y: 9,
                width: t * 2,
                height: t,
            },
            Region {
                x: t,
                y: 0,
                width: t,
                height: t * 3,
            },
        ]);
        assert!(index.intersects([0., 0., 1., 1.]));
        assert!(index.intersects([t as f32, (t * 2) as f32, (t + 1) as f32, (t * 3) as f32]));
        assert!(!index.intersects([0., (t * 2) as f32, t as f32, (t * 3) as f32]));
        assert!(!index.intersects([-8., -8., -1., -1.]));
        assert!(!index.intersects([3., 3., 3., 3.]));
        assert!(index.intersects([f32::NAN, 0., 1., 1.]));
        assert!(!GroupDamage::new(&[]).intersects([0., 0., 1., 1.]));
    }
}
