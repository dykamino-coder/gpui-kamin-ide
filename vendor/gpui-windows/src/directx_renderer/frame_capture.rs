//! KaminIDE patch: readback владеет неизменяемым контекстом своего устройства.

use super::*;

impl DirectXRenderer {
    /// KaminIDE patch: copy the drawn back buffer to memory (`frame_capture`).
    pub(super) fn capture_frame(&mut self) -> Result<()> {
        let started = std::time::Instant::now();
        let (w, h) = (self.width, self.height);
        let desc = D3D11_TEXTURE2D_DESC {
            Width: w,
            Height: h,
            MipLevels: 1,
            ArraySize: 1,
            Format: RENDER_TARGET_FORMAT,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
        };
        let devices = self.dev();
        let render_target = self
            .res()
            .render_target
            .as_ref()
            .context("render target missing")?;
        let mut staging: Option<ID3D11Texture2D> = None;
        unsafe {
            devices
                .device
                .CreateTexture2D(&desc, None, Some(&mut staging))?
        };
        let staging = staging.context("staging texture")?;
        let ctx = &devices.device_context;
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
        ].into_iter().flatten().cloned().collect();
        shader_executables.sort();
        shader_executables.dedup();
        let context = Some(gpui::frame_capture::FrameContext {
            device: devices.context.clone(), shader_executables, target_format: target_desc.Format.0,
            sample_count: target_desc.SampleDesc.Count, sample_quality: target_desc.SampleDesc.Quality,
            gamma_ratios: self.font_info.gamma_ratios,
            grayscale_enhanced_contrast: self.font_info.grayscale_enhanced_contrast,
        });
        if let Some(sink) = &mut self.capture {
            sink.completed(gpui::frame_capture::CapturedFrame { width: w, height: h, bgra, context }, started.elapsed());
        }
        Ok(())
    }

}
