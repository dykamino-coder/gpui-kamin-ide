//! Sparse frame damage: stencil stores the union of regions for one scene replay.

use super::*;

#[derive(Default)]
pub(super) struct DamageMask {
    target: Option<(u32, u32, ID3D11Texture2D, ID3D11DepthStencilView)>,
    write: Option<ID3D11DepthStencilState>,
    read: Option<ID3D11DepthStencilState>,
    pub(super) active: bool,
}

impl DamageMask {
    /// Objects belong to the device; dimensions match the current frame target.
    pub(super) fn prepare(&mut self, device: &ID3D11Device, width: u32, height: u32) -> Result<()> {
        if self.write.is_none() || self.read.is_none() {
            let write = state(device, true)?;
            let read = state(device, false)?;
            (self.write, self.read) = (Some(write), Some(read));
        }
        if self
            .target
            .as_ref()
            .is_none_or(|(w, h, _, _)| (*w, *h) != (width, height))
        {
            let desc = D3D11_TEXTURE2D_DESC {
                Width: width,
                Height: height,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_D24_UNORM_S8_UINT,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_DEPTH_STENCIL.0 as u32,
                ..Default::default()
            };
            let mut texture = None;
            unsafe { device.CreateTexture2D(&desc, None, Some(&mut texture))? };
            let texture = texture.context("damage stencil texture missing")?;
            let mut view = None;
            unsafe { device.CreateDepthStencilView(&texture, None, Some(&mut view))? };
            self.target = Some((
                width,
                height,
                texture,
                view.context("damage stencil view missing")?,
            ));
        }
        Ok(())
    }

    pub(super) fn view(&self) -> Option<&ID3D11DepthStencilView> {
        self.active
            .then(|| self.target.as_ref().map(|(_, _, _, v)| v))
            .flatten()
    }

    pub(super) fn mark(&self, dc: &ID3D11DeviceContext) {
        // Clear stencil once; each clear draw marks its region with one.
        unsafe {
            dc.ClearDepthStencilView(self.view().unwrap(), D3D11_CLEAR_STENCIL.0 as u32, 1.0, 0);
            dc.OMSetDepthStencilState(self.write.as_ref(), 1);
        }
    }

    pub(super) fn test(&self, dc: &ID3D11DeviceContext) {
        unsafe { dc.OMSetDepthStencilState(self.read.as_ref(), 1) };
    }

    pub(super) fn texture(&self) -> Option<&ID3D11Texture2D> {
        self.target.as_ref().map(|(_, _, texture, _)| texture)
    }
}

fn state(device: &ID3D11Device, write: bool) -> Result<ID3D11DepthStencilState> {
    let face = D3D11_DEPTH_STENCILOP_DESC {
        StencilFailOp: D3D11_STENCIL_OP_KEEP,
        StencilDepthFailOp: D3D11_STENCIL_OP_KEEP,
        StencilPassOp: if write {
            D3D11_STENCIL_OP_REPLACE
        } else {
            D3D11_STENCIL_OP_KEEP
        },
        StencilFunc: if write {
            D3D11_COMPARISON_ALWAYS
        } else {
            D3D11_COMPARISON_EQUAL
        },
    };
    let desc = D3D11_DEPTH_STENCIL_DESC {
        DepthEnable: false.into(),
        DepthWriteMask: D3D11_DEPTH_WRITE_MASK_ZERO,
        DepthFunc: D3D11_COMPARISON_ALWAYS,
        StencilEnable: true.into(),
        StencilReadMask: u8::MAX,
        StencilWriteMask: if write { u8::MAX } else { 0 },
        FrontFace: face,
        BackFace: face,
    };
    let mut result = None;
    unsafe { device.CreateDepthStencilState(&desc, Some(&mut result))? };
    result.context("damage stencil state missing")
}

impl DirectXRenderer {
    /// Intermediate group/path targets do not use the frame damage mask.
    pub(super) fn current_depth(&self) -> Option<&ID3D11DepthStencilView> {
        if self.group_target.is_none() {
            self.retained.mask.view()
        } else {
            None
        }
    }

    pub(super) fn bind_current_target(&self) {
        unsafe {
            self.dev()
                .device_context
                .OMSetRenderTargets(Some(self.current_target()), self.current_depth());
        }
    }
}
