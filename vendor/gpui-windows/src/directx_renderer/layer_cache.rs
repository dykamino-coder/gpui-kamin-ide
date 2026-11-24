//! KaminIDE patch: instance buffers of retained scene layers (`gpui::SceneLayer`).
//!
//! A frame draws a layer (`PrimitiveBatch::Layer`) from buffers kept per
//! layer and filled again only when the layer's version changes: an
//! unchanged layer costs its draw calls, not an upload. The buffers are
//! swapped into the pipelines for the layer's batches, so every batch is
//! drawn by the same code as the frame's own. They are a cache of the layer:
//! a layer not drawn in a frame loses them, and a lost device drops all.

use super::{DirectXRenderer, PipelineState, create_buffer, create_buffer_view};
use anyhow::Result;
use collections::HashMap;
use gpui::Scene;
use windows::Win32::Graphics::Direct3D11::{ID3D11Buffer, ID3D11Device, ID3D11ShaderResourceView};

/// One pipeline's instance buffer outside the pipeline.
struct Instances {
    buffer: ID3D11Buffer,
    view: Option<ID3D11ShaderResourceView>,
    size: usize,
    gradients: Option<super::analytic_gradient::GradientBuffers>,
}

impl Instances {
    fn new<T>(device: &ID3D11Device) -> Result<Self> {
        let buffer = create_buffer(device, std::mem::size_of::<T>(), 1)?;
        let view = create_buffer_view(device, &buffer)?;
        Ok(Self { buffer, view, size: 1, gradients: None })
    }

    fn swap<T>(&mut self, pipeline: &mut PipelineState<T>) {
        std::mem::swap(&mut self.buffer, &mut pipeline.buffer);
        std::mem::swap(&mut self.view, &mut pipeline.view);
        std::mem::swap(&mut self.size, &mut pipeline.buffer_size);
        std::mem::swap(&mut self.gradients, &mut pipeline.gradients);
    }
}

/// The buffers of one layer: shadows, quads, underlines, monochrome,
/// subpixel and polychrome sprites, and the version they hold.
struct LayerBuffers {
    sets: [Instances; 6],
    version: Option<u64>,
    frame: u64,
}

/// Buffers of the layers drawn in the last frame, by layer id.
#[derive(Default)]
pub(super) struct LayerCache {
    layers: HashMap<u64, LayerBuffers>,
    frame: u64,
}

impl LayerCache {
    pub(super) fn gradient_memory(&self, sink: &mut dyn FnMut(&'static str, u64, u64)) {
        for layer in self.layers.values() {
            for set in &layer.sets {
                if let Some(gradients) = &set.gradients { gradients.gpu_memory(sink); }
            }
        }
    }

    /// A new frame: buffers of layers the last one did not draw are dropped.
    pub(super) fn begin_frame(&mut self) {
        let last = self.frame;
        self.layers.retain(|_, layer| layer.frame == last);
        self.frame += 1;
    }

    /// The buffers of a layer the last frame drew and this one has not yet,
    /// emptied for another layer to fill.
    fn take_stale(&mut self) -> Option<LayerBuffers> {
        let frame = self.frame;
        let id = *self.layers.iter().find(|(_, layer)| layer.frame != frame)?.0;
        let mut buffers = self.layers.remove(&id)?;
        buffers.version = None;
        Some(buffers)
    }

    /// The device was lost: every buffer with it.
    pub(super) fn clear(&mut self) {
        self.layers.clear();
    }
}

impl DirectXRenderer {
    fn swap_layer_buffers(&mut self, sets: &mut [Instances; 6]) {
        let p = &mut self.pipelines;
        sets[0].swap(&mut p.shadow_pipeline);
        sets[1].swap(&mut p.quad_pipeline);
        sets[2].swap(&mut p.underline_pipeline);
        sets[3].swap(&mut p.mono_sprites);
        sets[4].swap(&mut p.subpixel_sprites);
        sets[5].swap(&mut p.poly_sprites);
    }

    fn new_layer_buffers(&self) -> Result<LayerBuffers> {
        let device = &self.devices.as_ref().ok_or_else(|| anyhow::anyhow!("devices missing"))?.device;
        Ok(LayerBuffers {
            sets: [
                Instances::new::<gpui::Shadow>(device)?,
                Instances::new::<gpui::Quad>(device)?,
                Instances::new::<gpui::Underline>(device)?,
                Instances::new::<gpui::MonochromeSprite>(device)?,
                Instances::new::<gpui::SubpixelSprite>(device)?,
                Instances::new::<gpui::PolychromeSprite>(device)?,
            ],
            version: None,
            frame: 0,
        })
    }

    /// Draw layer `index` of `scene` from its buffers, filling them first
    /// when the layer changed since they were.
    pub(super) fn draw_layer(&mut self, scene: &Scene, index: usize) -> Result<()> {
        let Some(layer) = scene.layer(index) else {
            return Ok(());
        };
        let mut buffers = match self.layer_cache.layers.remove(&layer.id()) {
            Some(buffers) => buffers,
            // A layer recorded again in place of one the last frame drew takes
            // over that layer's buffers, already grown to its size: a rebuilt
            // document costs an upload, not new device buffers.
            None => match self.layer_cache.take_stale() {
                Some(buffers) => buffers,
                None => self.new_layer_buffers()?,
            },
        };
        self.swap_layer_buffers(&mut buffers.sets);
        let fresh = buffers.version == Some(layer.version());
        let drawn = match fresh {
            true => Ok(()),
            false => self.upload_scene_buffers(layer.scene()),
        }
        .and_then(|()| self.draw_scene_at(layer.scene(), Some([0; 6])));
        self.swap_layer_buffers(&mut buffers.sets);
        buffers.version = drawn.is_ok().then_some(layer.version());
        buffers.frame = self.layer_cache.frame;
        self.layer_cache.layers.insert(layer.id(), buffers);
        drawn
    }
}
