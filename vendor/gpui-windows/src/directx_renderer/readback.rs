//! KaminIDE patch: копия цели кадра в CPU-видимую текстуру и чтение её строк.
//!
//! Staging-текстура живёт ровно одну копию: между кадрами окно не удерживает
//! CPU-видимую память размером с кадр.

use super::*;

/// Поставить в очередь GPU копию `target` в новую staging-текстуру.
/// Возвращает текстуру и размер; копия ещё может выполняться.
pub(super) fn copy_to_staging(
    devices: &DirectXRendererDevices,
    target: &ID3D11Texture2D,
) -> Result<(ID3D11Texture2D, u32, u32)> {
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
    let texture = texture.context("staging texture")?;
    unsafe { devices.device_context.CopyResource(&texture, target) };
    Ok((texture, desc.Width, desc.Height))
}

/// Прочитать BGRA-строки staging-текстуры без padding. `Map` ждёт копию,
/// если GPU её ещё не выполнил.
pub(super) fn read_staging(
    context: &ID3D11DeviceContext,
    staging: &ID3D11Texture2D,
    width: u32,
    height: u32,
) -> Result<Vec<u8>> {
    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    unsafe { context.Map(staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))? };
    let row_bytes = width as usize * 4;
    let mut pixels = vec![0; row_bytes * height as usize];
    // SAFETY: Map завершён, staging создана как BGRA8 width x height; каждая
    // строка содержит RowPitch >= row_bytes доступных байтов до Unmap. Буфер
    // pixels отдельный и вмещает height строк по row_bytes.
    unsafe {
        for row in 0..height as usize {
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

/// Синхронное чтение текущего содержимого `target`.
pub(super) fn read(devices: &DirectXRendererDevices, target: &ID3D11Texture2D) -> Result<Vec<u8>> {
    let (staging, width, height) = copy_to_staging(devices, target)?;
    read_staging(&devices.device_context, &staging, width, height)
}

#[cfg(test)]
pub(super) mod tests {
    use super::{
        D3D11_SUBRESOURCE_DATA, D3D11_TEXTURE2D_DESC, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC,
        DirectXDevices, DirectXRendererDevices, ID3D11Texture2D, RENDER_TARGET_FORMAT, read,
    };

    pub(in crate::directx_renderer) fn target(
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
    fn each_read_returns_the_current_target_at_odd_width() {
        let device = DirectXDevices::new().unwrap();
        let devices = DirectXRendererDevices::new(&device, false).unwrap();
        assert_eq!(read(&devices, &target(&devices, 3, 2, 17)).unwrap(), vec![17; 24]);
        assert_eq!(read(&devices, &target(&devices, 3, 2, 93)).unwrap(), vec![93; 24]);
        assert_eq!(read(&devices, &target(&devices, 5, 4, 2)).unwrap(), vec![2; 80]);
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
        assert!(read(&devices, &unsupported.unwrap()).is_err());
    }
}
