//! KaminIDE patch: a run of overlapping group composites in one pass.
//!
//! A blend mode mixed in the shader reads a copy of the backdrop, and a
//! composite overlapping an earlier one must see it already blended, so the
//! composite levels of `group_batch` needed one level per overlap: a chain of N
//! overlapping groups (effects-480: every layer overlaps its neighbours) cost N
//! backdrop copies, N draws and N read-after-write barriers. As in the fine
//! stage of vello (per-tile command lists, every pixel runs its tile's list
//! in order), the run is binned into 16 px tiles of the target: each tile
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
//! and carry no mask image; the others keep the levels.

use super::*;

/// Tile side in target pixels (`GROUP_TILE` in shaders.hlsl).
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
    (r.x / TILE, r.y / TILE, last(r.x, r.width), last(r.y, r.height))
}

/// Bin the leading composites of `regions` on a target `width` x `height`:
/// how many are taken (stopping before one that would put more than `cap` in
/// a tile) and the lists: the first entry of every tile (one more than the
/// tiles), then composite instance indices (k + 1), document order per tile.
pub(super) fn bin(regions: &[Region], width: u32, height: u32, cap: u32, lists: &mut TileLists) -> usize {
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
    for t in 0..tiles {
        counts[t] = data[t];
    }
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
    /// Whether composite `s` can join a pass: a retained image and no mask image.
    pub(super) fn tiles_take(&self, s: &PaintSurface) -> bool {
        self.group_tiles
            && self.group_composite_quad(s).is_some()
            && self
                .blur
                .group_mask
                .get(s.group as usize - 1)
                .is_none_or(|m| m.is_none())
    }

    /// Draw the leading composites of `run` that `tiles_take` accepts, on one
    /// atlas page, in one pass; returns how many were drawn (0: fewer than two
    /// could share a pass, nothing drawn).
    pub(super) fn draw_composite_tiles(&mut self, run: &[&PaintSurface]) -> Result<usize> {
        let mut quads = vec![BlurQuad::default()];
        let mut regions = Vec::new();
        let mut page = None;
        for s in run.iter().take(MAX_RUN) {
            if !self.tiles_take(s) {
                break;
            }
            let Some((quad, p)) = self.group_composite_quad(s) else {
                break;
            };
            if *page.get_or_insert(p) != p {
                break;
            }
            quads.push(quad);
            regions.push(self.composite_region(s));
        }
        let mut lists = std::mem::take(&mut self.blur.tile_lists);
        let n = bin(&regions, self.width, self.height, TILE_CAP, &mut lists);
        let result = match page {
            Some(page) if n >= 2 => self.draw_tile_pass(&mut quads, &regions[..n], page, &mut lists),
            _ => Ok(()),
        };
        self.blur.tile_lists = lists;
        result.map(|()| if n >= 2 { n } else { 0 })
    }

    fn draw_tile_pass(
        &mut self,
        quads: &mut Vec<BlurQuad>,
        regions: &[Region],
        page: usize,
        lists: &mut TileLists,
    ) -> Result<()> {
        let device = self.dev().device.clone();
        let dc = self.dev().device_context.clone();
        let x0 = regions.iter().map(|r| r.x).min().unwrap_or(0);
        let y0 = regions.iter().map(|r| r.y).min().unwrap_or(0);
        let x1 = regions.iter().map(|r| r.x + r.width).max().unwrap_or(0);
        let y1 = regions.iter().map(|r| r.y + r.height).max().unwrap_or(0);
        let cover = Region {
            x: x0,
            y: y0,
            width: x1 - x0,
            height: y1 - y0,
        };
        quads.truncate(regions.len() + 1);
        quads[0].bounds = cover.floats();
        quads[0].poly_count = self.width.div_ceil(TILE);
        log::debug!(
            target: "gpui::frame",
            "tile pass {} composites cover {}x{} lists {} entries",
            regions.len(),
            cover.width,
            cover.height,
            lists.data.len()
        );
        // One copy of the run's rectangle: every pixel starts from it.
        self.copy_level_backdrop(slice::from_ref(&cover))?;
        let len = lists.data.len();
        if lists.buffer.as_ref().is_none_or(|b| b.2 < len) {
            let cap = len.next_power_of_two();
            let buffer = create_buffer(&device, std::mem::size_of::<u32>(), cap)?;
            let view = create_buffer_view(&device, &buffer)?;
            lists.buffer = Some((buffer, view, cap));
        }
        let (buffer, view, _) = lists.buffer.as_ref().unwrap();
        update_buffer(&dc, buffer, &lists.data)?;
        self.ensure_blur_globals(&device)?;
        update_buffer(
            &dc,
            self.blur.globals[0].as_ref().unwrap(),
            &[global_params(self.font_info, [self.width as f32, self.height as f32])],
        )?;
        let target = self.current_target().to_vec();
        self.pipelines.group_tiles_pipeline.update_buffer(&device, &dc, quads)?;
        unsafe {
            dc.PSSetShaderResources(4, Some(slice::from_ref(view)));
            dc.OMSetRenderTargets(Some(&target), None);
        }
        self.pipelines.group_tiles_pipeline.draw_with_texture_blended(
            &dc,
            &self.blur.groups[page].srv,
            slice::from_ref(&self.res().viewport),
            &self.blur.globals,
            slice::from_ref(&self.globals.sampler),
            1,
            None,
        )?;
        unsafe {
            dc.PSSetShaderResources(2, Some(&[None]));
            dc.PSSetShaderResources(4, Some(&[None]));
        }
        lists.passes += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Region, TILE_CAP, TileLists, bin};

    fn r(x: u32, y: u32, width: u32, height: u32) -> Region {
        Region { x, y, width, height }
    }

    #[test]
    fn every_tile_lists_its_composites_in_document_order() {
        let mut l = TileLists::default();
        // 40 x 20 target: 3 x 2 tiles of 16 px.
        let n = bin(&[r(0, 0, 20, 10), r(10, 5, 30, 15), r(33, 17, 1, 1)], 40, 20, TILE_CAP, &mut l);
        assert_eq!(n, 3);
        let tile = |t: usize| &l.data[l.data[t] as usize..l.data[t + 1] as usize];
        assert_eq!(tile(0), [1, 2]);
        assert_eq!(tile(1), [1, 2]);
        assert_eq!(tile(2), [2]);
        assert_eq!(tile(3), [2]);
        assert_eq!(tile(4), [2]);
        assert_eq!(tile(5), [2, 3]);
        assert_eq!(l.data.len(), 7 + 9);
    }

    #[test]
    fn a_full_tile_ends_the_pass_before_the_composite_that_overflows_it() {
        let mut l = TileLists::default();
        let stack = vec![r(0, 0, 8, 8); 5];
        assert_eq!(bin(&stack, 32, 32, 3, &mut l), 3);
        assert_eq!(&l.data[l.data[0] as usize..l.data[1] as usize], [1, 2, 3]);
        // Disjoint composites never split.
        let apart: Vec<Region> = (0..4).map(|i| r(i * 16, 0, 16, 16)).collect();
        assert_eq!(bin(&apart, 64, 16, 1, &mut l), 4);
    }
}
