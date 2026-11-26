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
//! and a mask image view (or have none).

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

mod bins;
use super::cover_shape::CoverShape;
pub(super) use bins::TileLists;
#[cfg(test)]
use bins::bin;

impl DirectXRenderer {
    /// Whether composite `s` can join a pass: it has a retained image.
    pub(super) fn tiles_take(&self, s: &PaintSurface) -> bool {
        self.group_tiles && self.group_composite_quad(s).is_some()
    }

    /// Draw the leading composites of `run` that `tiles_take` accepts, on one
    /// atlas page and with one mask image (or none), in one pass; returns how
    /// many were drawn (0: none could, nothing drawn). A composite no other
    /// can join is a pass of one: drawn by the adapter's blender instead, its
    /// partly covered pixels (an antialiased clip edge, a translucent run)
    /// could land a code away from the same composite drawn in a pass, so a
    /// frame would change where an unrelated primitive between two
    /// composites splits their run (album-cover: a fixed box painted over
    /// the page moved the clip-path edges of two groups by one code).
    pub(super) fn draw_composite_tiles(&mut self, run: &[&PaintSurface]) -> Result<usize> {
        let mut quads = vec![BlurQuad::default()];
        let mut regions = Vec::new();
        let mut shapes = Vec::new();
        type View = Option<Option<ID3D11ShaderResourceView>>;
        let mut first: Option<(usize, View)> = None;
        for s in run.iter().take(MAX_RUN) {
            let Some((mut quad, p)) = self.group_composite_quad(s).filter(|_| self.group_tiles) else {
                break;
            };
            let mask = self.blur.group_mask.get(s.group as usize - 1).cloned().flatten();
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
            // A source-over composite leaves a pixel it does not colour
            // exactly as it was, so it is listed only where its shape can
            // colour; a blend mode mixes even a transparent source, so its
            // whole region stays listed.
            let (vw, vh) = (self.width as f32, self.height as f32);
            shapes.push((quad.blend_mode == 0).then(|| CoverShape::of(&quad, vw, vh)).flatten());
            quads.push(quad);
            regions.push(self.composite_region(s));
        }
        let mut lists = std::mem::take(&mut self.blur.tile_lists);
        let taken = bins::bin(&regions, &shapes, self.width, self.height, TILE_CAP, &mut lists);
        if taken == 0 {
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
        shapes.truncate(taken);
        if let Some(w) = self.blur.frame_within {
            let keep: Vec<usize> = (0..taken).filter(|&k| w.meets(regions[k])).collect();
            if keep.len() < taken {
                regions = keep.iter().map(|&k| regions[k]).collect();
                shapes = keep.iter().map(|&k| shapes[k]).collect();
                quads = std::iter::once(quads[0]).chain(keep.iter().map(|&k| quads[k + 1])).collect();
                bins::bin(&regions, &shapes, self.width, self.height, TILE_CAP, &mut lists);
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
        // Within a scissor only its pixels are drawn: the cover (backdrop copy
        // and the pass quad) need not reach farther.
        let (x0, y0, x1, y1) = match self.blur.frame_within {
            Some(w) => {
                let (x, y) = (x0.max(w.x), y0.max(w.y));
                (x, y, x1.min(w.x + w.width).max(x), y1.min(w.y + w.height).max(y))
            }
            None => (x0, y0, x1, y1),
        };
        let cover = Region {
            x: x0,
            y: y0,
            width: x1 - x0,
            height: y1 - y0,
        };
        // Only the tiles that list a composite are drawn; the spans follow
        // the lists in the same buffer (`group_tiles_vertex`).
        let spans = bins::spans(&lists.data, self.width, cover);
        if spans.is_empty() {
            lists.passes += 1;
            return Ok(());
        }
        quads.truncate(regions.len() + 1);
        quads[0].bounds = cover.floats();
        quads[0].poly_count = self.width.div_ceil(TILE);
        quads[0].blend_mode = lists.data.len() as u32;
        lists.data.extend(spans.iter().flat_map(|s| [s.x, s.y, s.width, s.height]));
        log::debug!(
            target: "gpui::frame",
            "tile pass {} composites cover {}x{} spans {} lists {} entries",
            regions.len(),
            cover.width,
            cover.height,
            spans.len(),
            lists.data.len()
        );
        // One copy of the drawn spans: every pixel starts from it.
        self.copy_level_backdrop(&spans)?;
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
            dc.VSSetShaderResources(4, Some(slice::from_ref(view)));
            dc.PSSetShaderResources(4, Some(slice::from_ref(view)));
            dc.OMSetRenderTargets(Some(&target), None);
        }
        self.pipelines.group_tiles_pipeline.draw_with_texture_blended(
            &dc,
            &self.blur.groups[page].srv,
            slice::from_ref(&self.res().viewport),
            &self.blur.globals,
            slice::from_ref(&self.globals.sampler),
            spans.len() as u32,
            None,
        )?;
        unsafe {
            dc.PSSetShaderResources(2, Some(&[None]));
            dc.VSSetShaderResources(4, Some(&[None]));
            dc.PSSetShaderResources(4, Some(&[None]));
        }
        lists.passes += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
