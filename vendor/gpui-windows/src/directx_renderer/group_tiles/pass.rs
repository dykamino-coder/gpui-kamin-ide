//! Draw one ordered composite run from its coarse or bounded fine lists.

use super::*;

impl DirectXRenderer {
    pub(super) fn draw_tile_pass(
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
                (
                    x,
                    y,
                    x1.min(w.x + w.width).max(x),
                    y1.min(w.y + w.height).max(y),
                )
            }
            None => (x0, y0, x1, y1),
        };
        let cover = Region {
            x: x0,
            y: y0,
            width: x1 - x0,
            height: y1 - y0,
        };
        quads.truncate(regions.len() + 1);
        quads[0].bounds = cover.floats();
        let capacity = lists.buffer.as_ref().map_or(0, |b| b.2);
        let grid = self
            .blur
            .frame_within
            .filter(|_| regions.len() > 1)
            .and_then(|_| refine::bin_cover(regions, cover, capacity, lists))
            .unwrap_or_else(|| refine::Grid::full(self.width));
        quads[0].src_origin = grid.origin.map(|n| n as f32);
        quads[0].poly_count = grid.cols;
        quads[0].texel = [1.0 / grid.side as f32, 0.0];
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
            &[global_params(
                self.font_info,
                [self.width as f32, self.height as f32],
            )],
        )?;
        let target = self.current_target().to_vec();
        self.pipelines
            .group_tiles_pipeline
            .update_buffer(&device, &dc, quads)?;
        unsafe {
            dc.PSSetShaderResources(4, Some(slice::from_ref(view)));
            dc.OMSetRenderTargets(Some(&target), None);
        }
        self.pipelines
            .group_tiles_pipeline
            .draw_with_texture_blended(
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
