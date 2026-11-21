//! Refine the damage grid without growing retained tile buffers.

use super::{Region, TILE, TileLists};

const SIDE: u32 = 8;

pub(super) struct Grid {
    pub(super) origin: [u32; 2],
    pub(super) cols: u32,
    pub(super) side: u32,
}

impl Grid {
    pub(super) fn full(width: u32) -> Self {
        Self {
            origin: [0, 0],
            cols: width.div_ceil(TILE),
            side: TILE,
        }
    }
}

fn span(r: Region, cover: Region, grid: &Grid) -> Option<[u32; 4]> {
    let (x, y) = (r.x.max(cover.x), r.y.max(cover.y));
    let (right, bottom) = (
        (r.x + r.width).min(cover.x + cover.width),
        (r.y + r.height).min(cover.y + cover.height),
    );
    (x < right && y < bottom).then(|| {
        [
            (x - grid.origin[0]) / SIDE,
            (y - grid.origin[1]) / SIDE,
            (right - 1 - grid.origin[0]) / SIDE,
            (bottom - 1 - grid.origin[1]) / SIDE,
        ]
    })
}

/// The 16 px grid already determined the pass boundaries and order. Its aligned
/// subdivision preserves the run limit. Only lists within damage change; the
/// shader still checks the conservative texture bounds. On refusal, data stays
/// valid for the coarse grid; counts is scratch storage.
pub(super) fn bin_cover(
    regions: &[Region],
    cover: Region,
    gpu_capacity: usize,
    lists: &mut TileLists,
) -> Option<Grid> {
    if cover.width == 0 || cover.height == 0 || gpu_capacity == 0 {
        return None;
    }
    let origin = [cover.x / SIDE * SIDE, cover.y / SIDE * SIDE];
    let cols = (cover.x + cover.width - origin[0]).div_ceil(SIDE);
    let rows = (cover.y + cover.height - origin[1]).div_ceil(SIDE);
    let tiles = cols as usize * rows as usize;
    if tiles > lists.counts.capacity() {
        return None;
    }
    let grid = Grid {
        origin,
        cols,
        side: SIDE,
    };
    let counts = &mut lists.counts;
    counts.clear();
    counts.resize(tiles, 0);
    for &r in regions {
        if let Some([x0, y0, x1, y1]) = span(r, cover, &grid) {
            for y in y0..=y1 {
                for x in x0..=x1 {
                    counts[(y * cols + x) as usize] += 1;
                }
            }
        }
    }
    let needed = tiles + 1 + counts.iter().map(|&n| n as usize).sum::<usize>();
    if needed > lists.data.capacity() || needed > gpu_capacity || needed > u32::MAX as usize {
        return None;
    }
    let data = &mut lists.data;
    data.clear();
    let mut at = tiles as u32 + 1;
    for &n in counts.iter() {
        data.push(at);
        at += n;
    }
    data.push(at);
    data.resize(needed, 0);
    counts.copy_from_slice(&data[..tiles]);
    for (k, &r) in regions.iter().enumerate() {
        if let Some([x0, y0, x1, y1]) = span(r, cover, &grid) {
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let t = (y * cols + x) as usize;
                    data[counts[t] as usize] = k as u32 + 1;
                    counts[t] += 1;
                }
            }
        }
    }
    Some(grid)
}

#[cfg(test)]
mod tests {
    use super::super::{TILE_CAP, bin};
    use super::*;

    fn r(x: u32, y: u32, width: u32, height: u32) -> Region {
        Region {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn subdivision_preserves_every_pixels_order_and_reuses_capacity() {
        let regions = [r(0, 0, 37, 30), r(11, 5, 21, 33), r(25, 17, 28, 20)];
        let cover = r(13, 7, 33, 27);
        let mut lists = TileLists::default();
        assert_eq!(bin(&regions, 128, 128, TILE_CAP, &mut lists), 3);
        let coarse = lists.data.clone();
        let capacity = (lists.counts.capacity(), lists.data.capacity());
        let grid = bin_cover(&regions, cover, capacity.1, &mut lists).unwrap();
        assert_eq!((grid.origin, grid.cols, grid.side), ([8, 0], 5, 8));
        assert_eq!(capacity, (lists.counts.capacity(), lists.data.capacity()));
        let entries =
            |data: &[u32], t: usize| data[data[t] as usize..data[t + 1] as usize].to_vec();
        let mut old_steps = 0;
        let mut new_steps = 0;
        for y in cover.y..cover.y + cover.height {
            for x in cover.x..cover.x + cover.width {
                let old = entries(&coarse, (y / TILE * 8 + x / TILE) as usize);
                let t = (y - grid.origin[1]) / SIDE * grid.cols + (x - grid.origin[0]) / SIDE;
                let new = entries(&lists.data, t as usize);
                old_steps += old.len();
                new_steps += new.len();
                assert!(new.iter().all(|n| old.contains(n)));
                let covers = |&n: &u32| regions[n as usize - 1].meets(r(x, y, 1, 1));
                assert_eq!(
                    old.into_iter().filter(covers).collect::<Vec<_>>(),
                    new.into_iter().filter(covers).collect::<Vec<_>>()
                );
            }
        }
        assert!(new_steps < old_steps);
    }

    #[test]
    fn insufficient_cpu_or_gpu_capacity_keeps_the_coarse_data() {
        let regions = [r(0, 0, 16, 16); 2];
        let mut lists = TileLists::default();
        bin(&regions, 16, 16, TILE_CAP, &mut lists);
        let original = lists.data.clone();
        let capacity = (lists.counts.capacity(), lists.data.capacity());
        assert!(bin_cover(&regions, r(0, 0, 16, 16), usize::MAX, &mut lists).is_none());
        assert_eq!(original, lists.data);
        assert_eq!(capacity, (lists.counts.capacity(), lists.data.capacity()));
        bin(&regions, 128, 128, TILE_CAP, &mut lists);
        let original = lists.data.clone();
        let capacity = (lists.counts.capacity(), lists.data.capacity());
        assert!(bin_cover(&regions, r(0, 0, 16, 16), 1, &mut lists).is_none());
        assert_eq!(original, lists.data);
        assert_eq!(capacity, (lists.counts.capacity(), lists.data.capacity()));
    }

    #[test]
    fn subdivision_keeps_the_coarse_run_limit() {
        let regions = [r(0, 0, 16, 16); 5];
        let mut lists = TileLists::default();
        let taken = bin(&regions, 128, 128, 3, &mut lists);
        assert_eq!(taken, 3);
        let capacity = lists.data.capacity();
        bin_cover(&regions[..taken], r(0, 0, 16, 16), capacity, &mut lists).unwrap();
        for t in 0..4 {
            assert_eq!(
                &lists.data[lists.data[t] as usize..lists.data[t + 1] as usize],
                [1, 2, 3]
            );
        }
    }

    #[test]
    fn insufficient_data_capacity_keeps_coarse_lists_with_enough_scratch_and_gpu() {
        let regions = [r(0, 0, 16, 16); 2];
        let mut lists = TileLists::default();
        bin(&regions, 16, 16, TILE_CAP, &mut lists);
        lists.counts.reserve(4);
        let original = lists.data.clone();
        let capacity = (lists.counts.capacity(), lists.data.capacity());
        // Four fine tiles need five offsets and eight group indices.
        assert!(capacity.0 >= 4 && capacity.1 < 13);
        assert!(bin_cover(&regions, r(0, 0, 16, 16), usize::MAX, &mut lists).is_none());
        assert_eq!(original, lists.data);
        assert_eq!(capacity, (lists.counts.capacity(), lists.data.capacity()));
    }

    #[test]
    fn empty_cover_or_missing_gpu_buffer_preserves_both_vectors() {
        let regions = [r(0, 0, 16, 16); 2];
        let mut lists = TileLists::default();
        bin(&regions, 128, 128, TILE_CAP, &mut lists);
        let original = (lists.counts.clone(), lists.data.clone());
        let capacity = (lists.counts.capacity(), lists.data.capacity());
        for (cover, gpu_capacity) in [
            (r(1, 1, 0, 8), usize::MAX),
            (r(1, 1, 8, 0), usize::MAX),
            (r(1, 1, 8, 8), 0),
        ] {
            assert!(bin_cover(&regions, cover, gpu_capacity, &mut lists).is_none());
            assert_eq!(original, (lists.counts.clone(), lists.data.clone()));
            assert_eq!(capacity, (lists.counts.capacity(), lists.data.capacity()));
        }
    }
}
