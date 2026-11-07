//! KaminIDE patch: batched group passes — layers of disjoint groups and composite levels.
//!
//! A frame with N filtered or blended groups used to run a chain of dependent
//! passes per group (draw → blur → composite with its own backdrop copy), so
//! render-target switches and read-after-write barriers grew with N. As in
//! WebRender's render-task graph (independent tasks of one pass share a target
//! and one batch) and Skia Graphite's draw passes, independent work is grouped:
//!
//! - **Layers.** Groups whose stage regions are pairwise disjoint are drawn into
//!   the frame-domain stage together; one instanced kernel draw moves all of them
//!   into their atlas rectangles. Each group still rasterizes in frame
//!   coordinates with the frame viewport, and its kernel reads only its own
//!   region, exactly the pixels it read alone.
//! - **Composite levels.** A composite depends only on earlier composites that
//!   overlap it. Within one run of consecutive group composites, composite j gets
//!   level 1 + the highest level of an earlier overlapping composite; composites
//!   of one level are pairwise disjoint, so one backdrop copy and one instanced
//!   draw serve the whole level, and every pixel sees the same backdrop as in
//!   document order (Compositing 1 §5: each group is blended with everything
//!   painted before it). Masked composites join the draw of their mask image.

use super::*;

/// Largest number of groups or composites batched together; bounds the
/// quadratic overlap tests.
const MAX_BATCH: usize = 1024;

pub(super) fn intersects(a: Region, b: Region) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}

/// Composite level of every rectangle: 0 without an earlier overlapping one,
/// otherwise one above the highest earlier overlapping level.
pub(super) fn levels(rects: &[Region]) -> Vec<u32> {
    let mut level: Vec<u32> = Vec::with_capacity(rects.len());
    for (j, &rect) in rects.iter().enumerate() {
        let l = rects[..j]
            .iter()
            .zip(&level)
            .filter(|(r, _)| intersects(**r, rect))
            .map(|(_, l)| l + 1)
            .max()
            .unwrap_or(0);
        level.push(l);
    }
    level
}

/// Scratch instance vectors of a stage layer (kept between frames for their capacity).
#[derive(Default)]
pub(super) struct LayerInstances {
    shadows: Vec<Shadow>,
    quads: Vec<Quad>,
    underlines: Vec<Underline>,
    mono: Vec<MonochromeSprite>,
    subpixel: Vec<SubpixelSprite>,
    poly: Vec<PolychromeSprite>,
}

impl LayerInstances {
    fn clear(&mut self) {
        self.shadows.clear();
        self.quads.clear();
        self.underlines.clear();
        self.mono.clear();
        self.subpixel.clear();
        self.poly.clear();
    }
}

/// First-fit layers of pairwise disjoint rectangles; each layer lists indices
/// in increasing order and holds at most `MAX_BATCH` of them.
pub(super) fn pack_layers(rects: &[Region]) -> Vec<Vec<usize>> {
    let mut layers: Vec<Vec<usize>> = Vec::new();
    for (k, &r) in rects.iter().enumerate() {
        let free = layers
            .iter()
            .position(|l| l.len() < MAX_BATCH && l.iter().all(|&j| !intersects(rects[j], r)));
        match free {
            Some(l) => layers[l].push(k),
            None => layers.push(vec![k]),
        }
    }
    layers
}

/// Whether group `g` can join a stage layer: its image is final once drawn
/// (no nested composites or backdrops reading the stage) and its filter is
/// none or a single-pass kernel.
pub(super) fn joins_layer(g: &gpui::PaintGroup) -> bool {
    g.scene.surfaces.is_empty()
        && (g.blur_radius <= 0.0
            || gaussian::direct_radius(g.blur_radius)
                .is_some_and(|r| r <= gaussian::MAX_SINGLE_PASS_RADIUS))
}

impl DirectXRenderer {
    /// Draw the joinable groups `segment` (indices into `groups`) layer by layer
    /// and empty it. Groups are independent images, so their drawing order is
    /// free: first-fit puts each group into the first layer whose stage regions
    /// it does not overlap (graph colouring of the overlap graph), which needs
    /// about as many layers as the deepest overlap instead of one per
    /// overlapping neighbour in document order.
    pub(super) fn draw_group_layers(
        &mut self,
        groups: &[gpui::PaintGroup],
        segment: &mut Vec<usize>,
    ) -> Result<()> {
        let raws: Vec<Region> = segment
            .iter()
            .map(|&i| Region::for_group(&groups[i], self.width, self.height)[0])
            .collect();
        for layer in pack_layers(&raws) {
            let members: Vec<usize> = layer.iter().map(|&k| segment[k]).collect();
            // One upload per instance kind for the whole layer; on failure (an
            // oversized layer) every group uploads its own scene as before.
            let bases = self.upload_layer(groups, &members).ok();
            for (n, &k) in layer.iter().enumerate() {
                let i = segment[k];
                let base = bases.as_ref().map(|b| b[n]);
                self.draw_group_content(&groups[i], raws[k], base)?;
                self.blur.layer.push(i);
            }
            self.flush_group_layer(groups)?;
        }
        segment.clear();
        Ok(())
    }
    /// Upload the instances of every group of a layer, kind by kind, into one
    /// buffer each (WebRender and Graphite fill per-frame GPU buffers in bulk
    /// instead of mapping per draw). Returns each group's first index per kind.
    pub(super) fn upload_layer(
        &mut self,
        groups: &[gpui::PaintGroup],
        members: &[usize],
    ) -> Result<Vec<[usize; 6]>> {
        let mut all = std::mem::take(&mut self.blur.layer_instances);
        all.clear();
        let mut bases = Vec::with_capacity(members.len());
        for &i in members {
            let s = &groups[i].scene;
            bases.push([
                all.shadows.len(),
                all.quads.len(),
                all.underlines.len(),
                all.mono.len(),
                all.subpixel.len(),
                all.poly.len(),
            ]);
            all.shadows.extend_from_slice(&s.shadows);
            all.quads.extend_from_slice(&s.quads);
            all.underlines.extend_from_slice(&s.underlines);
            all.mono.extend_from_slice(&s.monochrome_sprites);
            all.subpixel.extend_from_slice(&s.subpixel_sprites);
            all.poly.extend_from_slice(&s.polychrome_sprites);
        }
        let result = self.upload_instances(&all);
        all.clear();
        self.blur.layer_instances = all;
        result.map(|()| bases)
    }

    fn upload_instances(&mut self, all: &LayerInstances) -> Result<()> {
        let device = self.dev().device.clone();
        let dc = self.dev().device_context.clone();
        let p = &mut self.pipelines;
        if !all.shadows.is_empty() {
            p.shadow_pipeline.update_buffer(&device, &dc, &all.shadows)?;
        }
        if !all.quads.is_empty() {
            p.quad_pipeline.update_buffer(&device, &dc, &all.quads)?;
        }
        if !all.underlines.is_empty() {
            p.underline_pipeline.update_buffer(&device, &dc, &all.underlines)?;
        }
        if !all.mono.is_empty() {
            p.mono_sprites.update_buffer(&device, &dc, &all.mono)?;
        }
        if !all.subpixel.is_empty() {
            p.subpixel_sprites.update_buffer(&device, &dc, &all.subpixel)?;
        }
        if !all.poly.is_empty() {
            p.poly_sprites.update_buffer(&device, &dc, &all.poly)?;
        }
        Ok(())
    }

    /// Move every group of the pending layer from the stage into the atlas:
    /// unfiltered ones by copy, filtered ones with one kernel draw per atlas page.
    pub(super) fn flush_group_layer(&mut self, groups: &[gpui::PaintGroup]) -> Result<()> {
        if self.blur.layer.is_empty() {
            return Ok(());
        }
        let layer = std::mem::take(&mut self.blur.layer);
        let mut pages: Vec<(usize, Vec<BlurQuad>)> = Vec::new();
        for &i in &layer {
            let g = &groups[i];
            let Some(r) = gaussian::direct_radius(g.blur_radius) else {
                self.save_group_stage(i);
                continue;
            };
            let (region, place) = (self.blur.group_regions[i], self.blur.group_place[i]);
            // The kernel reads only the group's own stage region: outside it the
            // stage held transparent black when the group was drawn alone.
            let raw = Region::for_group(g, self.width, self.height)[0];
            let quad = gaussian::kernel_quad(
                [place.x as f32, place.y as f32, region.width as f32, region.height as f32],
                [
                    region.x as f32 - place.x as f32,
                    region.y as f32 - place.y as f32,
                ],
                raw.floats(),
                6.0,
                [0.0; 2],
                r,
                gaussian::kernel_poly(g.blur_radius, r),
            );
            match pages.iter_mut().find(|(p, _)| *p == place.page) {
                Some((_, quads)) => quads.push(quad),
                None => pages.push((place.page, vec![quad])),
            }
        }
        self.blur.layer = layer;
        self.blur.layer.clear();
        let stage = self
            .blur
            .stage
            .as_ref()
            .context("group stage missing")?
            .srv
            .clone();
        for (page, quads) in pages {
            let target = &self.blur.groups[page];
            let (rtv, size) = (target.rtv.clone(), (target.width, target.height));
            self.kernel_instances(&stage, &rtv, size, &quads)?;
        }
        self.clear_group_stage()
    }

    /// Whether a surface can be drawn as an instance of a composite level.
    pub(super) fn composite_batches(&self, s: &PaintSurface) -> bool {
        s.group > 0
            && self
                .blur
                .group_slots
                .get(s.group as usize - 1)
                .is_some_and(|&slot| slot < self.blur.group_place.len())
    }

    /// Draw a run of consecutive group composites level by level.
    pub(super) fn draw_composite_run(&mut self, run: &[&PaintSurface]) -> Result<()> {
        for chunk in run.chunks(MAX_BATCH) {
            if let [s] = chunk {
                self.draw_group_composite(s)?;
                continue;
            }
            self.draw_composite_levels(chunk)?;
        }
        Ok(())
    }

    fn draw_composite_levels(&mut self, run: &[&PaintSurface]) -> Result<()> {
        let device = self.dev().device.clone();
        let dc = self.dev().device_context.clone();
        // (instance, atlas page, composite region, mask image key); masked
        // composites carry their mask rectangle per instance and share a draw
        // with the composites of the same mask image (the mask cache hands
        // equal images one view).
        let mut items = Vec::with_capacity(run.len());
        let mut masks: Vec<Option<ID3D11ShaderResourceView>> = Vec::new();
        for s in run {
            if let Some((mut quad, page)) = self.group_composite_quad(s) {
                let mask = self
                    .blur
                    .group_mask
                    .get(s.group as usize - 1)
                    .cloned()
                    .flatten();
                let key = match &mask {
                    Some(([srv], rect, once)) => {
                        quad.pad2 = [1.0, *once];
                        quad.mask_rect = *rect;
                        match masks.iter().position(|m| m == srv) {
                            Some(k) => k + 1,
                            None => {
                                masks.push(srv.clone());
                                masks.len()
                            }
                        }
                    }
                    None => 0,
                };
                items.push((quad, page, self.composite_region(s), key));
            }
        }
        let rects: Vec<Region> = items.iter().map(|i| i.2).collect();
        let level = levels(&rects);
        let top = level.iter().copied().max().unwrap_or(0);
        self.ensure_blur_globals(&device)?;
        update_buffer(
            &dc,
            self.blur.globals[0].as_ref().unwrap(),
            &[global_params(self.font_info, [self.width as f32, self.height as f32])],
        )?;
        let (premultiplied, replace) = (self.premultiplied_blend()?, self.replace_blend()?);
        let target = self.current_target().to_vec();
        let mut quads = Vec::new();
        for l in 0..=top {
            let members: Vec<usize> = (0..items.len()).filter(|&i| level[i] == l).collect();
            let blended: Vec<Region> = members
                .iter()
                .filter(|&&i| items[i].0.blend_mode != 0)
                .map(|&i| items[i].2)
                .collect();
            if !blended.is_empty() {
                self.copy_level_backdrop(&blended)?;
            }
            // One draw per (atlas page, blend state, mask image); members are disjoint.
            let mut keys: Vec<(usize, bool, usize)> = members
                .iter()
                .map(|&i| (items[i].1, items[i].0.blend_mode == 0, items[i].3))
                .collect();
            keys.sort_unstable();
            keys.dedup();
            for (page, normal, mask) in keys {
                quads.clear();
                quads.extend(
                    members
                        .iter()
                        .filter(|&&i| {
                            items[i].1 == page
                                && (items[i].0.blend_mode == 0) == normal
                                && items[i].3 == mask
                        })
                        .map(|&i| items[i].0),
                );
                self.pipelines
                    .blur_pipeline
                    .update_buffer(&device, &dc, &quads)?;
                if mask > 0 {
                    unsafe { dc.PSSetShaderResources(3, Some(slice::from_ref(&masks[mask - 1]))) };
                }
                unsafe { dc.OMSetRenderTargets(Some(&target), None) };
                self.pipelines.blur_pipeline.draw_with_texture_blended(
                    &dc,
                    &self.blur.groups[page].srv,
                    slice::from_ref(&self.res().viewport),
                    &self.blur.globals,
                    slice::from_ref(&self.globals.sampler),
                    quads.len() as u32,
                    Some(if normal { &premultiplied } else { &replace }),
                )?;
            }
            if !blended.is_empty() {
                unsafe { dc.PSSetShaderResources(2, Some(&[None])) };
            }
        }
        Ok(())
    }

    /// Copy the backdrop under the blended composites of one level into the
    /// frame-sized level backdrop at the same coordinates: one copy of their
    /// bounding box when it is compact, otherwise one copy per composite.
    fn copy_level_backdrop(&mut self, rects: &[Region]) -> Result<()> {
        let device = self.dev().device.clone();
        let dc = self.dev().device_context.clone();
        let (w, h) = (self.width, self.height);
        if self
            .blur
            .level_backdrop
            .as_ref()
            .is_none_or(|t| t.width != w || t.height != h)
        {
            self.blur.level_backdrop = Some(create_blur_texture(&device, w, h, false)?);
        }
        let x0 = rects.iter().map(|r| r.x).min().unwrap_or(0);
        let y0 = rects.iter().map(|r| r.y).min().unwrap_or(0);
        let x1 = rects.iter().map(|r| r.x + r.width).max().unwrap_or(0);
        let y1 = rects.iter().map(|r| r.y + r.height).max().unwrap_or(0);
        let area = |r: &Region| u64::from(r.width) * u64::from(r.height);
        let union = Region {
            x: x0,
            y: y0,
            width: x1 - x0,
            height: y1 - y0,
        };
        let copies = if 4 * area(&union) <= 5 * rects.iter().map(area).sum::<u64>() {
            vec![union]
        } else {
            rects.to_vec()
        };
        let source = self
            .group_target_tex
            .as_ref()
            .or(self.res().render_target.as_ref())
            .context("render target missing")?;
        let copy = self.blur.level_backdrop.as_ref().unwrap();
        unsafe {
            for r in copies {
                let area = D3D11_BOX {
                    left: r.x,
                    top: r.y,
                    front: 0,
                    right: r.x + r.width,
                    bottom: r.y + r.height,
                    back: 1,
                };
                dc.CopySubresourceRegion(&copy.texture, 0, r.x, r.y, 0, source, 0, Some(&area));
            }
            dc.PSSetShaderResources(2, Some(&copy.srv));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
