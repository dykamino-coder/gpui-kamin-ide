//! KaminIDE patch: readback владеет неизменяемым контекстом своего устройства.

use super::*;

impl DirectXRenderer {
    /// KaminIDE patch: copy the drawn back buffer to memory (`frame_capture`).
    pub(super) fn capture_frame(&mut self) -> Result<()> {
        let started = std::time::Instant::now();
        let (w, h) = (self.width, self.height);
        // KaminIDE patch: one staging texture per size (`retained.rs`).
        let staging = self.capture_staging()?;
        let devices = self.dev();
        let render_target = self
            .res()
            .render_target
            .as_ref()
            .context("render target missing")?;
        let ctx = &devices.device_context;
        // KaminIDE patch: the frame is complete when the GPU has executed it;
        // the copy below is the capture's own cost, reported apart (`copied`).
        wait_gpu_idle(&devices.device, ctx)?;
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

}

/// KaminIDE patch: block until the GPU has executed every command submitted
/// so far (an event query), without touching the frame's pixels.
fn wait_gpu_idle(device: &ID3D11Device, ctx: &ID3D11DeviceContext) -> Result<()> {
    use windows::core::Interface;
    let desc = D3D11_QUERY_DESC { Query: D3D11_QUERY_EVENT, MiscFlags: 0 };
    let mut query = None;
    unsafe { device.CreateQuery(&desc, Some(&mut query))? };
    let query: ID3D11Query = query.context("event query missing")?;
    unsafe { ctx.End(&query) };
    let mut flags = 0u32;
    loop {
        let mut done = 0i32;
        // The `GetData` wrapper maps S_FALSE ("not yet") to Ok; read the raw HRESULT.
        let hr = unsafe {
            (Interface::vtable(ctx).GetData)(
                Interface::as_raw(ctx),
                Interface::as_raw(&query),
                &mut done as *mut i32 as *mut _,
                std::mem::size_of::<i32>() as u32,
                flags,
            )
        };
        hr.ok()?;
        if hr == windows::Win32::Foundation::S_OK && done != 0 {
            return Ok(());
        }
        // The first call flushed the queue; later polls only ask.
        flags = D3D11_ASYNC_GETDATA_DONOTFLUSH.0 as u32;
        std::thread::yield_now();
    }
}
