//! KaminIDE patch: the frame clear is a draw, not `ClearRenderTargetView`.
//!
//! On Intel Iris Xe (driver 32.0.101.7085) the hardware clear of the frame
//! target is not reliably ordered before the draws that follow it: about one
//! frame in twenty of an unchanged scene came out with a single wrong pixel
//! inside a glyph, at a random place. The scene was byte-identical between the
//! right and the wrong frame, two readbacks of the same target agreed, and the
//! wrong colour (per-channel coverage 0.89 / 0.56 / 0.40 for one monochrome
//! sprite) is not one a sprite can blend. Zero-initialised or padded atlas
//! tiles and point sampling kept the fault; drawing the clear removed it
//! (0 of 130 processes against 4.4 % with the hardware clear). WebRender works
//! around the same driver behaviour by clearing its caches with quads
//! (`webrender/src/renderer/init.rs:172`, `clear_caches_with_quads`).
//!
//! The clear is a strip over the whole target with blending off, so the
//! target takes the colour exactly as `ClearRenderTargetView` wrote it: the
//! pixel shader output converts to UNORM like the clear value does. Its cost
//! was below the noise on both adapters (warm frames of text-200 and a page of
//! many paragraphs, 10 interleaved pairs on Intel Iris Xe and RTX 4080), so it
//! replaces the hardware clear on every adapter.

use super::*;

/// The colour of the frame clear (`clear_colors` in `shaders.hlsl`).
#[derive(Clone, Copy)]
#[repr(C)]
pub(super) struct ClearColor {
    color: [f32; 4],
}

impl DirectXRenderer {
    /// Fill the bound frame target with `color`: one draw with blending off,
    /// after the target, its viewport and the frame parameters are bound.
    pub(super) fn clear_frame(&mut self, color: [f32; 4]) -> Result<()> {
        let devices = self.devices.as_ref().context("devices missing")?;
        let pipeline = &mut self.pipelines.clear_pipeline;
        pipeline.update_buffer(
            &devices.device,
            &devices.device_context,
            &[ClearColor { color }],
        )?;
        pipeline.draw(
            &devices.device_context,
            D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
            4,
            1,
        )
    }
}
