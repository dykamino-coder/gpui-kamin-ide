//! KaminIDE patch: a run of overlapping group composites in one pass.
//!
//! A blend mode mixed in the shader reads a copy of the backdrop, and a
//! composite overlapping an earlier one must see it already blended, so the
//! composite levels of `group_batch` needed one level per overlap: a chain of N
//! overlapping groups (effects-480: every layer overlaps its neighbours) cost N
//! backdrop copies, N draws and N read-after-write barriers. As in the fine
//! stage of vello (per-tile command lists, every pixel runs its tile's list
//! in order), the run is split on 16 px tiles of the target: each tile
//! lists, in document order, the composites whose region meets it. One copy of
//! the run's rectangle is the starting backdrop and one draw over it applies,
//! per pixel, every composite covering the pixel in order, rounding to the
//! 8-bit target after every step (`group_tiles_fragment`): the canonical group
//! arithmetic of design.md, the steps of the copy path, all in the shader: the
//! result does not depend on the adapter's blender.
//!
//! Lists are bounded: a tile holds at most `TILE_CAP` composites, and the run
//! is split before the composite that would exceed it (one more copy and draw
//! per split, never a longer list). Composites of one pass share an atlas page
//! and a mask image view (or have none).

//! Within damage, the grid refines to 8 px without growing retained buffers.
//! Pass boundaries, composite order and per-step arithmetic stay unchanged.

use super::*;

mod pass;
mod refine;

/// Coarse tile side in target pixels.
pub(super) const TILE: u32 = 16;
/// Most composites one tile lists, so most loop steps per pixel.
pub(super) const TILE_CAP: u32 = 128;
/// Most composites of one pass (instances 1..; instance 0 is the cover).
const MAX_RUN: usize = 1023;

/// `GPUI_GROUP_TILES=0` keeps every run on the composite levels (comparison runs).
pub(super) fn enabled() -> bool {
    std::env::var_os("GPUI_GROUP_TILES").is_none_or(|v| v != "0")
}

/// Grow-only buffer of the tile lists and its scratch.
#[derive(Default)]
pub(super) struct TileLists {
    buffer: Option<(ID3D11Buffer, Option<ID3D11ShaderResourceView>, usize)>,
    counts: Vec<u32>,
    data: Vec<u32>,
    /// Passes drawn since the renderer was made (tests).
    pub(super) passes: u32,
}

/// Tiles of `r` on a grid of `cols` columns: (first column, first row, last
/// column, last row), inclusive.
fn tile_span(r: Region) -> (u32, u32, u32, u32) {
    let last = |o: u32, s: u32| (o + s.max(1) - 1) / TILE;
    (
        r.x / TILE,
        r.y / TILE,
        last(r.x, r.width),
        last(r.y, r.height),
    )
}

/// Bin the leading composites of `regions` on a target `width` x `height`:
/// how many are taken (stopping before one that would put more than `cap` in
/// a tile) and the lists: the first entry of every tile (one more than the
/// tiles), then composite instance indices (k + 1), document order per tile.
pub(super) fn bin(
    regions: &[Region],
    width: u32,
    height: u32,
    cap: u32,
    lists: &mut TileLists,
) -> usize {
    let cols = width.div_ceil(TILE);
    let tiles = (cols * height.div_ceil(TILE)) as usize;
    let counts = &mut lists.counts;
    counts.clear();
    counts.resize(tiles, 0);
    let mut taken = 0;
    for &r in regions {
        let (x0, y0, x1, y1) = tile_span(r);
        let full = (y0..=y1).any(|y| (x0..=x1).any(|x| counts[(y * cols + x) as usize] >= cap));
        if full {
            break;
        }
        for y in y0..=y1 {
            for x in x0..=x1 {
                counts[(y * cols + x) as usize] += 1;
            }
        }
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
        let (x0, y0, x1, y1) = tile_span(r);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let t = (y * cols + x) as usize;
                data[counts[t] as usize] = k as u32 + 1;
                counts[t] += 1;
            }
        }
    }
    taken
}

impl DirectXRenderer {
    /// Whether composite `s` can join a pass: it has a retained image.
    pub(super) fn tiles_take(&self, s: &PaintSurface) -> bool {
        self.group_tiles && self.group_composite_quad(s).is_some()
    }

    /// Draw the leading composites of `run` that `tiles_take` accepts, on one
    /// atlas page and with one mask image (or none), in one pass; returns how
    /// many were drawn (0: fewer than two could share a pass, nothing drawn).
    pub(super) fn draw_composite_tiles(&mut self, run: &[&PaintSurface]) -> Result<usize> {
        let mut quads = vec![BlurQuad::default()];
        let mut regions = Vec::new();
        type View = Option<Option<ID3D11ShaderResourceView>>;
        let mut first: Option<(usize, View)> = None;
        for s in run.iter().take(MAX_RUN) {
            let Some((mut quad, p)) = self.group_composite_quad(s).filter(|_| self.group_tiles)
            else {
                break;
            };
            let mask = self
                .blur
                .group_mask
                .get(s.group as usize - 1)
                .cloned()
                .flatten();
            // Masked composites carry their mask rectangle per instance and share
            // the pass with those of the same view (the mask cache hands equal
            // images one view), as in the composite levels.
            let view = mask.map(|([srv], rect, once)| {
                quad.pad2 = [1.0, once];
                quad.mask_rect = rect;
                srv
            });
            let (page, pass_view) = first.get_or_insert_with(|| (p, view.clone()));
            if *page != p || *pass_view != view {
                break;
            }
            quads.push(quad);
            regions.push(self.composite_region(s));
        }
        let mut lists = std::mem::take(&mut self.blur.tile_lists);
        let taken = bin(&regions, self.width, self.height, TILE_CAP, &mut lists);
        if taken < 2 {
            self.blur.tile_lists = lists;
            return Ok(0);
        }
        // KaminIDE patch: in a frame drawn within a rectangle the pass keeps
        // only the composites that reach it. The pass is the one the whole
        // frame draws (same split, same per-pixel steps); a composite outside
        // the scissor writes none of its pixels, so it costs no list entries,
        // no backdrop copy and no draw. A lone survivor stays in the pass:
        // drawn alone it would take another path than the whole frame.
        regions.truncate(taken);
        if let Some(w) = self.blur.frame_within {
            let keep: Vec<usize> = (0..taken).filter(|&k| w.meets(regions[k])).collect();
            if keep.len() < taken {
                regions = keep.iter().map(|&k| regions[k]).collect();
                quads = std::iter::once(quads[0])
                    .chain(keep.iter().map(|&k| quads[k + 1]))
                    .collect();
                bin(&regions, self.width, self.height, TILE_CAP, &mut lists);
            }
        }
        let result = match first {
            Some((page, view)) if !regions.is_empty() => {
                if let Some(v) = &view {
                    let dc = self.dev().device_context.clone();
                    unsafe { dc.PSSetShaderResources(3, Some(slice::from_ref(v))) };
                }
                self.draw_tile_pass(&mut quads, &regions, page, &mut lists)
            }
            _ => Ok(()),
        };
        self.blur.tile_lists = lists;
        result.map(|()| taken)
    }
}

#[cfg(test)]
mod tests;
