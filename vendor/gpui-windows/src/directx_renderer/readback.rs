//! Один ленивый staging-буфер: размер, формат и устройство ограничивают повторное использование.

use super::*;

#[derive(Default)]
pub(super) struct Readback {
    entry: Option<Staging>,
}

struct Staging {
    device: ID3D11Device,
    width: u32,
    height: u32,
    format: DXGI_FORMAT,
    texture: ID3D11Texture2D,
}

impl Readback {
    pub(super) fn read(
        &mut self,
        devices: &DirectXRendererDevices,
        target: &ID3D11Texture2D,
    ) -> Result<Vec<u8>> {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe { target.GetDesc(&mut desc) };
        anyhow::ensure!(
            desc.Format == RENDER_TARGET_FORMAT
                && desc.MipLevels == 1
                && desc.ArraySize == 1
                && desc.SampleDesc.Count == 1
                && desc.SampleDesc.Quality == 0,
            "readback requires a single-sample, single-layer BGRA8 target",
        );
        let reuse = self.entry.as_ref().is_some_and(|entry| {
            entry.device == devices.device
                && entry.width == desc.Width
                && entry.height == desc.Height
                && entry.format == desc.Format
        });
        if !reuse {
            // Старый размер не остаётся в пуле: одновременно хранится один буфер.
            self.entry = None;
            let staging_desc = D3D11_TEXTURE2D_DESC {
                Usage: D3D11_USAGE_STAGING,
                BindFlags: 0,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                MiscFlags: 0,
                ..desc
            };
            let mut texture = None;
            unsafe {
                devices
                    .device
                    .CreateTexture2D(&staging_desc, None, Some(&mut texture))?
            };
            self.entry = Some(Staging {
                device: devices.device.clone(),
                width: desc.Width,
                height: desc.Height,
                format: desc.Format,
                texture: texture.context("staging texture")?,
            });
        }
        let staging = &self
            .entry
            .as_ref()
            .context("staging texture missing")?
            .texture;
        let context = &devices.device_context;
        unsafe { context.CopyResource(staging, target) };
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe { context.Map(staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))? };
        let row_bytes = desc.Width as usize * 4;
        let mut pixels = vec![0; row_bytes * desc.Height as usize];
        // SAFETY: Map завершён, формат цели BGRA8; каждая строка содержит
        // row_bytes доступных байтов до Unmap. Буфер pixels отдельный.
        unsafe {
            for row in 0..desc.Height as usize {
                std::ptr::copy_nonoverlapping(
                    (mapped.pData as *const u8).add(row * mapped.RowPitch as usize),
                    pixels.as_mut_ptr().add(row * row_bytes),
                    row_bytes,
                );
            }
            context.Unmap(staging, 0);
        }
        Ok(pixels)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        D3D11_SUBRESOURCE_DATA, D3D11_TEXTURE2D_DESC, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC,
        DirectXDevices, DirectXRendererDevices, ID3D11Texture2D, RENDER_TARGET_FORMAT, Readback,
    };

    fn target(
        devices: &DirectXRendererDevices,
        width: u32,
        height: u32,
        byte: u8,
    ) -> ID3D11Texture2D {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: RENDER_TARGET_FORMAT,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            ..Default::default()
        };
        let bytes = vec![byte; width as usize * height as usize * 4];
        let initial = D3D11_SUBRESOURCE_DATA {
            pSysMem: bytes.as_ptr().cast(),
            SysMemPitch: width * 4,
            SysMemSlicePitch: 0,
        };
        let mut texture = None;
        unsafe {
            devices
                .device
                .CreateTexture2D(&desc, Some(&initial), Some(&mut texture))
        }
        .unwrap();
        texture.unwrap()
    }

    #[test]
    fn same_size_reuses_staging_but_reads_each_new_target_at_odd_width() {
        let device = DirectXDevices::new().unwrap();
        let devices = DirectXRendererDevices::new(&device, false).unwrap();
        let mut readback = Readback::default();
        assert!(readback.entry.is_none());
        assert_eq!(
            readback
                .read(&devices, &target(&devices, 3, 2, 17))
                .unwrap(),
            vec![17; 24]
        );
        let first = readback.entry.as_ref().unwrap().texture.clone();
        assert_eq!(
            readback
                .read(&devices, &target(&devices, 3, 2, 93))
                .unwrap(),
            vec![93; 24]
        );
        assert_eq!(readback.entry.as_ref().unwrap().texture, first);
    }

    #[test]
    fn unsupported_format_is_rejected_before_staging_allocation() {
        let device = DirectXDevices::new().unwrap();
        let devices = DirectXRendererDevices::new(&device, false).unwrap();
        let source = target(&devices, 3, 2, 0);
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe { source.GetDesc(&mut desc) };
        desc.Format = DXGI_FORMAT_R8G8B8A8_UNORM;
        let mut unsupported = None;
        unsafe {
            devices
                .device
                .CreateTexture2D(&desc, None, Some(&mut unsupported))
        }
        .unwrap();
        let mut readback = Readback::default();
        assert!(readback.read(&devices, &unsupported.unwrap()).is_err());
        assert!(readback.entry.is_none());
    }

    #[test]
    fn resize_and_device_change_replace_the_single_staging_slot() {
        let a = DirectXDevices::new().unwrap();
        let b = DirectXDevices::new().unwrap();
        let a = DirectXRendererDevices::new(&a, false).unwrap();
        let b = DirectXRendererDevices::new(&b, false).unwrap();
        let mut readback = Readback::default();
        readback.read(&a, &target(&a, 3, 2, 1)).unwrap();
        let first = readback.entry.as_ref().unwrap().texture.clone();
        assert_eq!(
            readback.read(&a, &target(&a, 5, 4, 2)).unwrap(),
            vec![2; 80]
        );
        assert_ne!(readback.entry.as_ref().unwrap().texture, first);
        let resized = readback.entry.as_ref().unwrap().texture.clone();
        assert_eq!(
            readback.read(&b, &target(&b, 5, 4, 3)).unwrap(),
            vec![3; 80]
        );
        let entry = readback.entry.as_ref().unwrap();
        assert_ne!(entry.texture, resized);
        assert_eq!(unsafe { entry.texture.GetDevice() }.unwrap(), b.device);
    }
}
