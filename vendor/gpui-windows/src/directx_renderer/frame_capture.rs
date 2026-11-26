//! KaminIDE patch: readback владеет неизменяемым контекстом своего устройства.

use super::*;

impl DirectXRenderer {
    /// KaminIDE patch: copy the drawn back buffer to memory (`frame_capture`).
    pub(super) fn capture_frame(&mut self) -> Result<()> {
        let started = std::time::Instant::now();
        let (w, h) = (self.width, self.height);
        // KaminIDE patch: one staging texture per size (`retained.rs`).
        let staging = self.capture_staging()?;
        // KaminIDE patch: the frame is complete when the GPU has executed it;
        // the copy below is the capture's own cost, reported apart (`copied`).
        // The wait is reported apart too (`waited`): on a hardware adapter it
        // is GPU execution the window path never waits for (`present` is
        // DO_NOT_WAIT), on WARP it is the frame's CPU raster.
        let waited = std::time::Instant::now();
        self.wait_gpu_idle()?;
        let gpu_wait = waited.elapsed();
        let drawn = &self.retained.drawn;
        let report = gpui::frame_capture::RasterReport {
            plan: drawn.plan.to_string(),
            full_by: drawn.full_by.map(str::to_string),
            shift_miss: drawn.shift_miss.map(str::to_string),
            shift_compare: drawn.shift_stats.map(|s| s.to_string()),
            delta: drawn.delta,
            moved_px: drawn.moved_px,
            raster_px: drawn.rects.iter().map(|r| r[2] as u64 * r[3] as u64).sum(),
            rects: drawn.rects.clone(),
            gpu_wait,
        };
        if let Some(sink) = &mut self.capture {
            sink.waited(gpu_wait);
            sink.raster(report);
        }
        let devices = self.dev();
        let render_target = self
            .res()
            .render_target
            .as_ref()
            .context("render target missing")?;
        let ctx = &devices.device_context;
        let copy_started = std::time::Instant::now();
        unsafe { ctx.CopyResource(&staging, render_target) };
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe { ctx.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))? };
        let row = (w * 4) as usize;
        let mut bgra = vec![0u8; row * h as usize];
        for y in 0..h as usize {
            // KaminIDE patch: SAFETY: the mapped staging texture is w x h
            // BGRA8, so each of its h rows has RowPitch >= w * 4 readable
            // bytes until Unmap below.
            let src = unsafe {
                std::slice::from_raw_parts(
                    (mapped.pData as *const u8).add(y * mapped.RowPitch as usize),
                    row,
                )
            };
            bgra[y * row..(y + 1) * row].copy_from_slice(src);
        }
        unsafe { ctx.Unmap(&staging, 0) };
        // KaminIDE patch: capture retains the device generation used by this target.
        let mut target_desc = D3D11_TEXTURE2D_DESC::default();
        unsafe { render_target.GetDesc(&mut target_desc) };
        let mut shader_executables: Vec<_> = [
            &self.pipelines.shadow_pipeline.shader_executables,
            &self.pipelines.quad_pipeline.shader_executables,
            &self.pipelines.path_rasterization_pipeline.shader_executables,
            &self.pipelines.path_sprite_pipeline.shader_executables,
            &self.pipelines.underline_pipeline.shader_executables,
            &self.pipelines.mono_sprites.shader_executables,
            &self.pipelines.subpixel_sprites.shader_executables,
            &self.pipelines.poly_sprites.shader_executables,
            &self.pipelines.blur_pipeline.shader_executables,
            &self.pipelines.clear_pipeline.shader_executables,
        ].into_iter().flatten().cloned().collect();
        shader_executables.push(self.pipelines.quad_gradient.executable.clone());
        shader_executables.sort();
        shader_executables.dedup();
        let context = Some(gpui::frame_capture::FrameContext {
            device: devices.context.clone(), shader_executables, target_format: target_desc.Format.0,
            sample_count: target_desc.SampleDesc.Count, sample_quality: target_desc.SampleDesc.Quality,
            gamma_ratios: self.font_info.gamma_ratios,
            grayscale_enhanced_contrast: self.font_info.grayscale_enhanced_contrast,
        });
        if let Some(sink) = &mut self.capture {
            sink.copied(copy_started.elapsed());
            sink.completed(gpui::frame_capture::CapturedFrame { width: w, height: h, bgra, context }, started.elapsed());
        }
        Ok(())
    }

    /// The capture staging texture for the current size, created once per size.
    pub(super) fn capture_staging(&mut self) -> Result<ID3D11Texture2D> {
        let (w, h) = (self.width, self.height);
        if let Some((sw, sh, texture)) = &self.retained.staging
            && (*sw, *sh) == (w, h)
        {
            return Ok(texture.clone());
        }
        let mut desc = offscreen_target_desc(w, h);
        desc.Usage = D3D11_USAGE_STAGING;
        desc.BindFlags = 0;
        desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
        let mut staging: Option<ID3D11Texture2D> = None;
        unsafe { self.dev().device.CreateTexture2D(&desc, None, Some(&mut staging))? };
        let staging = staging.context("staging texture")?;
        self.retained.staging = Some((w, h, staging.clone()));
        Ok(staging)
    }
}
