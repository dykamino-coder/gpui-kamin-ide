//! Arbitrary stop buffers follow quad instances, including retained layer instances.
//! WARP executes the same analytic shader directly in the destination surface.
//! Analytic quads have a pixel program of their own (as WebRender separates
//! `brush_solid` from `brush_linear_gradient`): the plain quad program carries
//! neither the stop loop nor the t5/t6 buffers, so pages without gradients pay
//! nothing for them. Both programs read the same quad instance buffer.

use super::*;

/// The analytic-gradient pixel program over the quad instance buffer.
pub(super) struct GradientProgram {
    pub(super) fragment: ID3D11PixelShader,
    pub(super) executable: String,
}

impl GradientProgram {
    pub(super) fn new(device: &ID3D11Device) -> Result<Self> {
        let raw = RawShaderBytes::new(ShaderModule::QuadGradient, ShaderTarget::Fragment)?;
        Ok(Self {
            executable: super::shader_identity::executable(raw.as_bytes())?,
            fragment: create_fragment_shader(device, raw.as_bytes())?,
        })
    }
}

/// Splits `quads` (instances `first..`) into maximal runs of one program and
/// yields `(analytic, first_instance, count)` in paint order. Runs keep the
/// primitive order exactly, so splitting never reorders overlapping quads.
pub(super) fn program_runs(quads: &[Quad], first: usize) -> impl Iterator<Item = (bool, usize, usize)> + '_ {
    let mut at = 0;
    std::iter::from_fn(move || {
        let analytic = quads.get(at)?.is_analytic_gradient();
        let len = quads[at..].iter().take_while(|q| q.is_analytic_gradient() == analytic).count();
        let run = (analytic, first + at, len);
        at += len;
        Some(run)
    })
}

pub(super) struct GradientBuffers {
    descriptors: Buffer,
    stops: Buffer,
}

struct Buffer {
    buffer: ID3D11Buffer,
    view: Option<ID3D11ShaderResourceView>,
    capacity: usize,
    used: usize,
    stride: usize,
}

impl Buffer {
    fn new<T>(device: &ID3D11Device) -> Result<Self> {
        let buffer = create_buffer(device, std::mem::size_of::<T>(), 1)?;
        let view = create_buffer_view(device, &buffer)?;
        Ok(Self {
            buffer,
            view,
            capacity: 1,
            used: 0,
            stride: std::mem::size_of::<T>(),
        })
    }

    fn upload<T>(
        &mut self,
        device: &ID3D11Device,
        dc: &ID3D11DeviceContext,
        data: &[T],
    ) -> Result<()> {
        if data.is_empty() {
            return Ok(());
        }
        anyhow::ensure!(
            std::mem::size_of_val(data) <= MAX_INSTANCE_BUFFER_SIZE,
            "Analytic gradient buffer exceeds instance limit"
        );
        if data.len() > self.capacity {
            let capacity = data
                .len()
                .next_power_of_two()
                .min(MAX_INSTANCE_BUFFER_SIZE / std::mem::size_of::<T>());
            let buffer = create_buffer(device, std::mem::size_of::<T>(), capacity)?;
            let view = create_buffer_view(device, &buffer)?;
            *self = Self {
                buffer,
                view,
                capacity,
                used: 0,
                stride: std::mem::size_of::<T>(),
            };
        }
        self.used = data.len();
        update_buffer(dc, &self.buffer, data)
    }
}

impl GradientBuffers {
    pub(super) fn gpu_memory(&self, sink: &mut dyn FnMut(&'static str, u64, u64)) {
        for (path, b) in [
            ("gpu/gradient/descriptors", &self.descriptors),
            ("gpu/gradient/stops", &self.stops),
        ] {
            sink(
                path,
                (b.capacity * b.stride) as u64,
                (b.used * b.stride) as u64,
            );
        }
    }
}

impl<T> PipelineState<T> {
    pub(super) fn upload_gradients(
        &mut self,
        device: &ID3D11Device,
        dc: &ID3D11DeviceContext,
        data: &gpui::GradientData,
    ) -> Result<()> {
        if data.descriptors.is_empty() {
            self.gradients = None;
            return Ok(());
        }
        if self.gradients.is_none() {
            self.gradients = Some(GradientBuffers {
                descriptors: Buffer::new::<gpui::GradientDescriptor>(device)?,
                stops: Buffer::new::<gpui::GradientStop>(device)?,
            });
        }
        let buffers = self.gradients.as_mut().unwrap();
        buffers.descriptors.upload(device, dc, &data.descriptors)?;
        buffers.stops.upload(device, dc, &data.stops)
    }

    pub(super) fn bind_gradients(&self, dc: &ID3D11DeviceContext) {
        if let Some(buffers) = &self.gradients {
            unsafe {
                dc.PSSetShaderResources(
                    5,
                    Some(&[buffers.descriptors.view.clone(), buffers.stops.view.clone()]),
                );
            }
        } else {
            unsafe {
                dc.PSSetShaderResources(5, Some(&[None, None]));
            }
        }
    }
}
