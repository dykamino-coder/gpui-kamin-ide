//! Reusable capture staging texture owned by the retained renderer.
use super::super::*;

impl DirectXRenderer {
    /// The capture staging texture for the current size, created once per size.
    pub(in crate::directx_renderer) fn capture_staging(&mut self) -> Result<ID3D11Texture2D> {
        let (w, h) = (self.width, self.height);
        if let Some((sw, sh, texture)) = &self.retained.staging
            && (*sw, *sh) == (w, h)
        {
            return Ok(texture.clone());
        }
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
        let mut staging: Option<ID3D11Texture2D> = None;
        unsafe {
            self.dev()
                .device
                .CreateTexture2D(&desc, None, Some(&mut staging))?
        };
        let staging = staging.context("staging texture")?;
        self.retained.staging = Some((w, h, staging.clone()));
        Ok(staging)
    }
}
