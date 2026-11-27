//! KaminIDE patch: a path batch costs the pixels its paths can reach, not the
//! window.
//!
//! Paths rasterize with 4x MSAA into a window-sized intermediate, and the
//! sprite pass then writes each covered pixel's samples, averaged, into the
//! target. Upstream cleared and resolved the whole intermediate for every
//! batch, and a frame redrawn within a rectangle did so for every path batch
//! of the scene: a fixed cost of the window's area times the batch count,
//! whatever the damage. Here a batch works within its area only: the pixels
//! its paths' clipped bounds cover, within the target and the bound scissor
//! (the redraw rectangle, a group's stage region). Only that area is cleared
//! and rasterized, a batch whose area is empty draws nothing, and the sprite
//! pass reads the samples itself (`path_sprite_fragment`), so the
//! intermediate has no resolved copy. Every pixel the sprites read lies in
//! the area just cleared and drawn: their rectangles are the clipped bounds,
//! or the union of them for a batch of several draw orders.

use super::*;
use windows::Win32::Foundation::RECT;

impl DirectXRenderer {
    /// Draw one path batch into the current target.
    pub(super) fn draw_paths(&mut self, paths: &[Path<ScaledPixels>]) -> Result<()> {
        let dc = self.dev().device_context.clone();
        let mut old = [RECT::default(); 16];
        let mut count = old.len() as u32;
        let state = unsafe { dc.RSGetState()? };
        let mut desc = D3D11_RASTERIZER_DESC::default();
        unsafe {
            state.GetDesc(&mut desc);
            dc.RSGetScissorRects(&mut count, Some(old.as_mut_ptr()));
        }
        let old = &old[..count as usize];
        let scissor = desc.ScissorEnable.as_bool().then(|| scissor_bounds(old));
        let area = path_area(paths, (self.width, self.height), scissor);
        if self.timing_main {
            let px = area.map_or(0.0, |a| f64::from(a.width) * f64::from(a.height));
            self.gpu_timer.instrumentation.batches.intermediate(px);
        }
        let Some(area) = area else {
            return Ok(());
        };
        self.ensure_path_target()?;
        let original = self.begin_scissor(area)?;
        let drawn = self
            .draw_paths_to_intermediate(paths, area)
            .and_then(|()| self.draw_paths_from_intermediate(paths));
        unsafe {
            dc.RSSetState(&original);
            dc.RSSetScissorRects(Some(old));
        }
        drawn
    }

    /// Clear `area` of the multisampled intermediate and rasterize the
    /// batch into it, under the scissor on `area`.
    fn draw_paths_to_intermediate(
        &mut self,
        paths: &[Path<ScaledPixels>],
        area: Region,
    ) -> Result<()> {
        let devices = self.devices.as_ref().context("devices missing")?;
        let resources = self.resources.as_ref().context("resources missing")?;
        let view = resources
            .path_intermediate_msaa_view
            .as_ref()
            .context("path view missing")?;
        let dc1: ID3D11DeviceContext1 = devices.device_context.cast()?;
        unsafe {
            devices
                .device_context
                .RSSetViewports(Some(slice::from_ref(&resources.viewport)));
            dc1.ClearView(view, &[0.0; 4], Some(&[area_rect(area)]));
            devices.device_context.OMSetRenderTargets(
                Some(slice::from_ref(&resources.path_intermediate_msaa_view)),
                None,
            );
        }
        let mut vertices = Vec::new();
        for path in paths {
            vertices.extend(path.vertices.iter().map(|v| {
                PathRasterizationSprite {
                    xy_position: v.xy_position,
                    st_position: v.st_position,
                    color: path.color,
                    bounds: path.clipped_bounds(),
                    color_bounds: path.color_mapping.map_or(path.clipped_bounds(), |m| m.0),
                    color_transform: path
                        .color_mapping
                        .map_or(TransformationMatrix::unit(), |m| m.1),
                }
            }));
        }
        self.pipelines.path_rasterization_pipeline.update_buffer(
            &devices.device,
            &devices.device_context,
            &vertices,
        )?;
        self.pipelines.path_rasterization_pipeline.draw(
            &devices.device_context,
            D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
            vertices.len() as u32,
            1,
        )?;
        unsafe {
            devices
                .device_context
                .OMSetRenderTargets(Some(self.current_target()), None);
            devices
                .device_context
                .RSSetViewports(Some(slice::from_ref(&self.res().viewport)));
        }
        Ok(())
    }

    fn draw_paths_from_intermediate(&mut self, paths: &[Path<ScaledPixels>]) -> Result<()> {
        let Some(first_path) = paths.first() else {
            return Ok(());
        };
        // When copying paths from the intermediate texture to the drawable,
        // each pixel must only be copied once, in case of transparent paths.
        //
        // If all paths have the same draw order, then their bounds are all
        // disjoint, so we can copy each path's bounds individually. If this
        // batch combines different draw orders, we perform a single copy
        // for a minimal spanning rect.
        let sprites = if paths.last().unwrap().order == first_path.order {
            paths
                .iter()
                .map(|path| PathSprite {
                    bounds: path.clipped_bounds(),
                })
                .collect::<Vec<_>>()
        } else {
            let mut bounds = first_path.clipped_bounds();
            for path in paths.iter().skip(1) {
                bounds = bounds.union(&path.clipped_bounds());
            }
            vec![PathSprite { bounds }]
        };
        let devices = self.devices.as_ref().context("devices missing")?;
        let resources = self.resources.as_ref().context("resources missing")?;
        self.pipelines.path_sprite_pipeline.update_buffer(
            &devices.device,
            &devices.device_context,
            &sprites,
        )?;
        let dc = &devices.device_context;
        unsafe {
            dc.PSSetShaderResources(
                PATH_SAMPLES_SLOT,
                Some(slice::from_ref(&resources.path_intermediate_srv)),
            )
        };
        let drawn = self.pipelines.path_sprite_pipeline.draw_with_texture(
            dc,
            &[None],
            slice::from_ref(&self.globals.sampler),
            sprites.len() as u32,
        );
        // The next batch renders into the intermediate: unbind it as an input.
        unsafe { dc.PSSetShaderResources(PATH_SAMPLES_SLOT, Some(&[None])) };
        drawn
    }
}

/// The shader register of the intermediate's samples (`t_path_samples`).
const PATH_SAMPLES_SLOT: u32 = 7;

fn area_rect(r: Region) -> RECT {
    RECT {
        left: r.x as i32,
        top: r.y as i32,
        right: (r.x + r.width) as i32,
        bottom: (r.y + r.height) as i32,
    }
}

/// The bounds of the bound scissor rectangles; `None` when they cover nothing.
fn scissor_bounds(rects: &[RECT]) -> Option<Region> {
    let r = rects
        .iter()
        .filter(|r| r.right > r.left && r.bottom > r.top)
        .copied()
        .reduce(|a, b| RECT {
            left: a.left.min(b.left),
            top: a.top.min(b.top),
            right: a.right.max(b.right),
            bottom: a.bottom.max(b.bottom),
        })?;
    let (x, y) = (r.left.max(0) as u32, r.top.max(0) as u32);
    Some(Region {
        x,
        y,
        width: (r.right.max(0) as u32).saturating_sub(x),
        height: (r.bottom.max(0) as u32).saturating_sub(y),
    })
}

/// The whole pixels the batch's clipped bounds reach, within the target of
/// `width` x `height` and the scissor; `None` when that is nothing. With the
/// scissor on but no rectangle bound, nothing passes it.
pub(super) fn path_area(
    paths: &[Path<ScaledPixels>],
    (width, height): (u32, u32),
    scissor: Option<Option<Region>>,
) -> Option<Region> {
    let bounds = paths
        .iter()
        .map(|p| p.clipped_bounds())
        .reduce(|a, b| a.union(&b))?;
    let [x0, y0, x1, y1] = [
        bounds.origin.x.0,
        bounds.origin.y.0,
        bounds.origin.x.0 + bounds.size.width.0,
        bounds.origin.y.0 + bounds.size.height.0,
    ];
    // Unknown bounds reach the whole target.
    let (mut left, mut top, mut right, mut bottom) = if [x0, y0, x1, y1].iter().any(|v| v.is_nan())
    {
        (0, 0, width, height)
    } else {
        let edge = |v: f32, limit: u32| v.clamp(0.0, limit as f32) as u32;
        (
            edge(x0.floor(), width),
            edge(y0.floor(), height),
            edge(x1.ceil(), width),
            edge(y1.ceil(), height),
        )
    };
    match scissor {
        None => {}
        Some(None) => return None,
        Some(Some(s)) => {
            left = left.max(s.x);
            top = top.max(s.y);
            right = right.min(s.x + s.width);
            bottom = bottom.min(s.y + s.height);
        }
    }
    (right > left && bottom > top).then(|| Region {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    })
}

#[cfg(test)]
#[path = "path_pass_tests.rs"]
mod tests;
