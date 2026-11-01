//! KaminIDE patch: ограниченный пул текстур и адресация выборки обрезанных групп.

use super::*;

#[derive(Default)]
pub(super) struct Pool(Vec<BlurTexture>, usize);

impl Pool {
    pub(super) fn take(&mut self, device: &ID3D11Device, w: u32, h: u32) -> Result<BlurTexture> {
        if let Some(i) = self.0.iter().position(|t| t.width == w && t.height == h) {
            let texture = self.0.swap_remove(i);
            self.1 -= texture.width as usize * texture.height as usize * 4;
            Ok(texture)
        } else {
            create_blur_texture(device, w, h, true)
        }
    }

    pub(super) fn put(&mut self, texture: BlurTexture) {
        const BUDGET: usize = 64 * 1024 * 1024;
        // Даже текстура 1x1 владеет объектами D3D: ограничиваем и их число.
        const MAX_IDLE: usize = 256;
        let bytes = texture.width as usize * texture.height as usize * 4;
        if bytes > BUDGET {
            return;
        }
        while self.1 + bytes > BUDGET || self.0.len() >= MAX_IDLE {
            let t = self.0.remove(0);
            self.1 -= t.width as usize * t.height as usize * 4;
        }
        self.1 += bytes;
        self.0.push(texture);
    }
}

// Полная ось сохраняет WRAP; за обрезанной осью исходный кадр прозрачен.
pub(super) fn sampler_index(rect: [f32; 4], frame: (f32, f32)) -> usize {
    usize::from(rect[2] < frame.0) | (usize::from(rect[3] < frame.1) << 1)
}

pub(super) fn samplers(device: &ID3D11Device) -> Result<[Option<ID3D11SamplerState>; 4]> {
    let mut states = [None, None, None, None];
    for (index, state) in states.iter_mut().enumerate() {
        let desc = D3D11_SAMPLER_DESC {
            Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
            AddressU: if index & 1 == 0 { D3D11_TEXTURE_ADDRESS_WRAP } else { D3D11_TEXTURE_ADDRESS_BORDER },
            AddressV: if index & 2 == 0 { D3D11_TEXTURE_ADDRESS_WRAP } else { D3D11_TEXTURE_ADDRESS_BORDER },
            AddressW: D3D11_TEXTURE_ADDRESS_WRAP,
            MaxAnisotropy: 1,
            ComparisonFunc: D3D11_COMPARISON_ALWAYS,
            MaxLOD: D3D11_FLOAT32_MAX,
            ..Default::default()
        };
        unsafe { device.CreateSamplerState(&desc, Some(state))?; }
    }
    Ok(states)
}

#[cfg(test)]
mod tests {
    use super::sampler_index;

    #[test]
    fn alternating_sizes_reuse_only_exact_texture_dimensions() -> anyhow::Result<()> {
        use super::Pool;
        use windows::{core::Interface, Win32::{Foundation::HMODULE, Graphics::{
            Direct3D::D3D_DRIVER_TYPE_WARP,
            Direct3D11::{D3D11CreateDevice, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION},
        }}};
        let mut device = None;
        unsafe {
            D3D11CreateDevice(None, D3D_DRIVER_TYPE_WARP, HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT, None, D3D11_SDK_VERSION,
                Some(&mut device), None, None)?;
        }
        let device = device.unwrap();
        let mut pool = Pool::default();
        let first = pool.take(&device, 71, 39)?;
        let retained = first.texture.clone();
        pool.put(first);
        let other = pool.take(&device, 70, 39)?;
        assert_eq!((other.width, other.height), (70, 39));
        assert_ne!(other.texture.as_raw(), retained.as_raw());
        pool.put(other);
        let reused = pool.take(&device, 71, 39)?;
        assert_eq!((reused.width, reused.height), (71, 39));
        assert_eq!(reused.texture.as_raw(), retained.as_raw());
        assert_eq!(pool.0.len(), 1);
        assert_eq!(pool.1, 70 * 39 * 4);
        Ok(())
    }

    #[test]
    fn only_cropped_axes_use_transparent_borders() {
        assert_eq!(sampler_index([0.0, 0.0, 800.0, 600.0], (800.0, 600.0)), 0);
        assert_eq!(sampler_index([12.0, 0.0, 200.0, 600.0], (800.0, 600.0)), 1);
        assert_eq!(sampler_index([0.0, 12.0, 800.0, 200.0], (800.0, 600.0)), 2);
        assert_eq!(sampler_index([12.0, 12.0, 200.0, 200.0], (800.0, 600.0)), 3);
    }
}
