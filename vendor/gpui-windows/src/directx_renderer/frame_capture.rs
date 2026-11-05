//! KaminIDE patch: readback владеет неизменяемым контекстом своего устройства.

use super::*;

impl DirectXRenderer {
    /// KaminIDE patch: copy the drawn back buffer to memory (`frame_capture`).
    pub(super) fn capture_frame(&mut self) -> Result<()> {
        let started = std::time::Instant::now();
        let (w, h) = (self.width, self.height);
        // KaminIDE patch: общий staging-путь сохраняет синхронный GPU readback.
        let devices = self.devices.as_ref().context("devices missing")?;
        let resources = self.resources.as_mut().context("resources missing")?;
        let render_target = resources
            .render_target
            .as_ref()
            .context("render target missing")?;
        let bgra = resources.readback.read(devices, render_target)?;
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
