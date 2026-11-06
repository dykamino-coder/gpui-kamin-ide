//! KaminIDE patch: асинхронный readback нарисованных кадров для `frame_capture`.
//!
//! Копия цели ставится в очередь GPU сразу за командами кадра, до Present;
//! пиксели забираются по сигналу GPU (`gpu_signal`), поэтому UI-поток не ждёт
//! ни выполнения кадра, ни компоновщика. Кадры доставляются sink строго по
//! порядку: новая копия сначала забирает незавершённую предыдущую.

use super::{gpu_signal::GpuSignal, *};
use std::time::Instant;

/// Копия кадра в очереди GPU вместе с неизменяемым контекстом его устройства.
pub(super) struct PendingReadback {
    token: usize,
    staging: ID3D11Texture2D,
    width: u32,
    height: u32,
    submitted: Instant,
    context: gpui::frame_capture::FrameContext,
    /// `None`: сигнал недоступен, копия забирается синхронно.
    _signal: Option<GpuSignal>,
}

impl DirectXRenderer {
    /// Поставить копию только что нарисованного кадра в очередь GPU.
    pub(super) fn queue_frame_readback(&mut self) -> Result<()> {
        self.finish_frame_readback(None);
        let submitted = Instant::now();
        let devices = self.devices.as_ref().context("devices missing")?;
        let render_target = self
            .resources
            .as_ref()
            .context("resources missing")?
            .render_target
            .as_ref()
            .context("render target missing")?;
        let (staging, width, height) = readback::copy_to_staging(devices, render_target)?;
        let context = self.frame_context(render_target);
        self.readback_token = self.readback_token.wrapping_add(1);
        let token = self.readback_token;
        let signal = GpuSignal::new(&devices.device, self.hwnd, token).log_err();
        let synchronous = signal.is_none();
        self.pending_readback = Some(PendingReadback {
            token,
            staging,
            width,
            height,
            submitted,
            context,
            _signal: signal,
        });
        if let Some(sink) = &mut self.capture {
            sink.submitted();
        }
        if synchronous {
            self.finish_frame_readback(None);
        }
        Ok(())
    }

    /// Забрать пиксели копии `token` (`None` — любой незавершённой) и отдать
    /// их sink. Чужой или устаревший `token` ничего не делает.
    pub(crate) fn finish_frame_readback(&mut self, token: Option<usize>) {
        if token.is_some_and(|token| {
            self.pending_readback
                .as_ref()
                .is_none_or(|pending| pending.token != token)
        }) {
            return;
        }
        let Some(pending) = self.pending_readback.take() else {
            return;
        };
        let PendingReadback {
            staging,
            width,
            height,
            submitted,
            context,
            ..
        } = pending;
        let read = match self.devices.as_ref() {
            Some(devices) => {
                let read = readback::read_staging(&devices.device_context, &staging, width, height);
                // Отпущенную копию runtime уничтожает в ближайшей точке flush;
                // без неё staging доживал бы до следующего кадра, а между
                // кадрами, как и при синхронном readback, копий быть не должно.
                drop(staging);
                unsafe { devices.device_context.Flush() };
                read
            }
            None => Err(anyhow::anyhow!("devices missing")),
        };
        let Some(sink) = &mut self.capture else {
            return;
        };
        match read {
            Ok(bgra) => {
                let frame = gpui::frame_capture::CapturedFrame {
                    width,
                    height,
                    bgra,
                    context: Some(context),
                };
                sink.completed(frame, submitted.elapsed());
            }
            Err(error) => {
                log::warn!("frame capture failed: {error}");
                sink.discarded();
            }
        }
    }

    /// Сбросить незавершённую копию без доставки (устройство потеряно или
    /// capture отключён); sink узнаёт, что отправленный кадр не придёт.
    pub(super) fn discard_frame_readback(&mut self) {
        if self.pending_readback.take().is_some()
            && let Some(sink) = &mut self.capture
        {
            sink.discarded();
        }
    }

    /// Устройство, формат цели и исполняемые шейдеры, которыми нарисован кадр.
    fn frame_context(&self, render_target: &ID3D11Texture2D) -> gpui::frame_capture::FrameContext {
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
        ]
        .into_iter()
        .flatten()
        .cloned()
        .collect();
        shader_executables.sort();
        shader_executables.dedup();
        gpui::frame_capture::FrameContext {
            device: self.dev().context.clone(),
            shader_executables,
            target_format: target_desc.Format.0,
            sample_count: target_desc.SampleDesc.Count,
            sample_quality: target_desc.SampleDesc.Quality,
            gamma_ratios: self.font_info.gamma_ratios,
            grayscale_enhanced_contrast: self.font_info.grayscale_enhanced_contrast,
        }
    }
}
