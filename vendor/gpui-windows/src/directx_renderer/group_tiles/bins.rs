//! KaminIDE patch: the tile lists of a pass of group composites and the
//! spans of tiles it draws.
//!
//! A composite is listed in a tile only where its convex cover shape
//! (`CoverShape`) meets the tile, and the pass draws only the row spans of
//! tiles that list something: a pixel of an empty tile would start from the
//! backdrop copy and end on it, so leaving it out changes no pixel. The
//! work of a pass then follows the pixels its composites can colour, not the
//! bounding box of the run (vello's coarse rasterizer: per-tile command
//! lists, empty tiles never dispatched).

use super::super::cover_shape::CoverShape;
use super::super::group_region::Region;
use super::TILE;

/// Grow-only buffer of the tile lists and its scratch.
#[derive(Default)]
pub(in super::super) struct TileLists {
    pub(super) buffer: Option<(
        windows::Win32::Graphics::Direct3D11::ID3D11Buffer,
        Option<windows::Win32::Graphics::Direct3D11::ID3D11ShaderResourceView>,
        usize,
    )>,
    pub(super) counts: Vec<u32>,
    pub(super) data: Vec<u32>,
    /// Passes drawn since the renderer was made (tests).
    pub(in super::super) passes: u32,
}

/// Tiles of `r` on a grid of `cols` columns: (first column, first row, last
/// column, last row), inclusive.
fn tile_span(r: Region) -> (u32, u32, u32, u32) {
    let last = |o: u32, s: u32| (o + s.max(1) - 1) / TILE;
    (r.x / TILE, r.y / TILE, last(r.x, r.width), last(r.y, r.height))
}

/// The tiles composite `k` is listed in: those of its region that its
/// shape (if any) meets.
fn each_tile(r: Region, shape: Option<&CoverShape>, mut f: impl FnMut(u32, u32)) {
    let (x0, y0, x1, y1) = tile_span(r);
    let tile = TILE as f32;
    for y in y0..=y1 {
        let (lo, hi) = match shape.map(|s| s.columns((y * TILE) as f32, ((y + 1) * TILE) as f32)) {
            None => (x0, x1),
            // A tile meets [lo, hi] when its left edge is at most hi and its
            // right edge at least lo (touching counts).
            Some(Some((lo, hi))) => {
                let lo = ((lo / tile).ceil() - 1.0).max(x0 as f32);
                let hi = (hi / tile).floor().min(x1 as f32);
                if !(lo <= hi) {
                    continue;
                }
                (lo as u32, hi as u32)
            }
            Some(None) => continue,
        };
        for x in lo..=hi {
            f(x, y);
        }
    }
}

/// Bin the leading composites of `regions` (with their cover `shapes`,
/// missing ones unbounded) on a target `width` x `height`: how many are
/// taken (stopping before one that would put more than `cap` in a tile) and
/// the lists: the first entry of every tile (one more than the tiles), then
/// composite instance indices (k + 1), document order per tile.
pub(in super::super) fn bin(
    regions: &[Region],
    shapes: &[Option<CoverShape>],
    width: u32,
    height: u32,
    cap: u32,
    lists: &mut TileLists,
) -> usize {
    let cols = width.div_ceil(TILE);
    let tiles = (cols * height.div_ceil(TILE)) as usize;
    let shape = |k: usize| shapes.get(k).and_then(Option::as_ref);
    let counts = &mut lists.counts;
    counts.clear();
    counts.resize(tiles, 0);
    let mut taken = 0;
    for (k, &r) in regions.iter().enumerate() {
        let mut full = false;
        each_tile(r, shape(k), |x, y| full |= counts[(y * cols + x) as usize] >= cap);
        if full {
            break;
        }
        each_tile(r, shape(k), |x, y| counts[(y * cols + x) as usize] += 1);
        taken += 1;
    }
    let data = &mut lists.data;
    data.clear();
    let mut at = tiles as u32 + 1;
    for &c in counts.iter() {
        data.push(at);
        at += c;
    }
    data.push(at);
    data.resize(at as usize, 0);
    // counts become each tile's write position.
    counts.copy_from_slice(&data[..tiles]);
    for (k, &r) in regions[..taken].iter().enumerate() {
        each_tile(r, shape(k), |x, y| {
            let t = (y * cols + x) as usize;
            data[counts[t] as usize] = k as u32 + 1;
            counts[t] += 1;
        });
    }
    taken
}

/// Row spans of the tiles that list a composite, on a target `width` wide,
/// cut to `cover`: the rectangles a pass draws and copies the backdrop of.
pub(in super::super) fn spans(data: &[u32], width: u32, cover: Region) -> Vec<Region> {
    let cols = width.div_ceil(TILE);
    let listed = |t: u32| data[t as usize + 1] > data[t as usize];
    let (cx1, cy1) = (cover.x + cover.width, cover.y + cover.height);
    let mut out = Vec::new();
    if cover.width == 0 || cover.height == 0 {
        return out;
    }
    let (x0, y0, x1, y1) = tile_span(cover);
    for y in y0..=y1 {
        let mut x = x0;
        while x <= x1 {
            if !listed(y * cols + x) {
                x += 1;
                continue;
            }
            let start = x;
            while x <= x1 && listed(y * cols + x) {
                x += 1;
            }
            let (left, top) = ((start * TILE).max(cover.x), (y * TILE).max(cover.y));
            let (right, bottom) = ((x * TILE).min(cx1), ((y + 1) * TILE).min(cy1));
            if right > left && bottom > top {
                out.push(Region {
                    x: left,
                    y: top,
                    width: right - left,
                    height: bottom - top,
                });
            }
        }
    }
    out
}
