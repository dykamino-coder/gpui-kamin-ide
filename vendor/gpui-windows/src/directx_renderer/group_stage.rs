//! KaminIDE patch: кадровый staging сохраняет точность GPU-интерполяции.

use super::*;
use windows::Win32::Foundation::RECT;

impl Region {
    fn rect(self) -> RECT {
        RECT {
            left: self.x as i32,
            top: self.y as i32,
            right: (self.x + self.width) as i32,
            bottom: (self.y + self.height) as i32,
        }
    }

    fn area(self) -> D3D11_BOX {
        D3D11_BOX {
            left: self.x,
            top: self.y,
            front: 0,
            right: self.x + self.width,
            bottom: self.y + self.height,
            back: 1,
        }
    }
}

impl DirectXRenderer {
    pub(super) fn clear_group_stage(&mut self) -> Result<()> {
        if let Some(dirty) = self.blur.stage_dirty.take() {
            let dc: ID3D11DeviceContext1 = self.dev().device_context.cast()?;
            let view = self.blur.stage.as_ref().unwrap().rtv[0].as_ref().unwrap();
            unsafe { dc.ClearView(view, &[0.0; 4], Some(&[dirty.rect()])) };
        }
        Ok(())
    }

    pub(super) fn begin_group_stage(&mut self, region: Region) -> Result<ID3D11RasterizerState> {
        let device = self.dev().device.clone();
        let dc = self.dev().device_context.clone();
        if self
            .blur
            .stage
            .as_ref()
            .is_none_or(|t| t.width != self.width || t.height != self.height)
        {
            let stage = create_blur_texture(&device, self.width, self.height, true)?;
            // Новый ресурс очищается один раз; следующие группы очищают только
            // предыдущую грязную область. Scissor ограничивает все записи в неё.
            unsafe { dc.ClearRenderTargetView(stage.rtv[0].as_ref().unwrap(), &[0.0; 4]) };
            self.blur.stage = Some(stage);
            self.blur.stage_dirty = None;
        }
        self.clear_group_stage()?;
        let original = unsafe { dc.RSGetState()? };
        if self.blur.stage_rasterizer.is_none() {
            let mut desc = D3D11_RASTERIZER_DESC::default();
            unsafe { original.GetDesc(&mut desc) };
            desc.ScissorEnable = true.into();
            unsafe { device.CreateRasterizerState(&desc, Some(&mut self.blur.stage_rasterizer))? };
        }
        unsafe {
            dc.RSSetState(self.blur.stage_rasterizer.as_ref());
            dc.RSSetScissorRects(Some(&[region.rect()]));
        }
        self.blur.stage_dirty = Some(region);
        Ok(original)
    }

    pub(super) fn save_group_stage(&self, slot: usize) {
        let stage = self.blur.stage.as_ref().unwrap();
        let region = self.blur.group_regions[slot];
        unsafe {
            self.dev().device_context.CopySubresourceRegion(
                &self.blur.groups[slot].texture,
                0,
                0,
                0,
                0,
                &stage.texture,
                0,
                Some(&region.area()),
            );
        }
    }
}

/// KaminIDE patch: временный target каскада имеет собственный домен scissor.
/// Состояние растеризатора остаётся прежним; после прохода возвращается клип группы.
pub(super) fn downsample_clip(
    dc: &ID3D11DeviceContext,
    width: u32,
    height: u32,
    draw: impl FnOnce() -> Result<()>,
) -> Result<()> {
    let mut old = [RECT::default(); 16];
    let mut count = old.len() as u32;
    unsafe {
        dc.RSGetScissorRects(&mut count, Some(old.as_mut_ptr()));
        dc.RSSetScissorRects(Some(&[Region::full(width, height).rect()]));
    }
    let result = draw();
    unsafe { dc.RSSetScissorRects(Some(&old[..count as usize])) };
    result
}

#[cfg(test)]
mod tests {
    use super::{Region, downsample_clip};
    use anyhow::Result;
    use windows::Win32::Foundation::RECT;

    #[test]
    fn copy_and_clear_use_the_same_half_open_frame_rectangle() {
        let r = Region {
            x: 403,
            y: 161,
            width: 71,
            height: 39,
        };
        let (clear, copy) = (r.rect(), r.area());
        assert_eq!(
            [clear.left, clear.top, clear.right, clear.bottom],
            [403, 161, 474, 200]
        );
        assert_eq!(
            [
                copy.left,
                copy.top,
                copy.right,
                copy.bottom,
                copy.front,
                copy.back
            ],
            [403, 161, 474, 200, 0, 1]
        );
    }
    #[test]
    fn cascade_clip_restores_all_rectangles_even_when_draw_fails() -> Result<()> {
        use windows::Win32::{
            Foundation::HMODULE,
            Graphics::{
                Direct3D::D3D_DRIVER_TYPE_WARP,
                Direct3D11::{
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice,
                },
            },
        };
        let (mut device, mut context) = (None, None);
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_WARP,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )?;
        }
        let dc = context.unwrap();
        let original = [
            Region {
                x: 403,
                y: 161,
                width: 71,
                height: 39,
            }
            .rect(),
            Region {
                x: 12,
                y: 20,
                width: 8,
                height: 10,
            }
            .rect(),
        ];
        unsafe { dc.RSSetScissorRects(Some(&original)) };
        let result = downsample_clip(&dc, 35, 19, || {
            let mut count = 1;
            let mut rect = RECT::default();
            unsafe { dc.RSGetScissorRects(&mut count, Some(&mut rect)) };
            assert_eq!(count, 1);
            assert_eq!(rect, Region::full(35, 19).rect());
            anyhow::bail!("draw failed")
        });
        assert!(result.is_err());
        let mut restored = [RECT::default(); 16];
        let mut count = restored.len() as u32;
        unsafe { dc.RSGetScissorRects(&mut count, Some(restored.as_mut_ptr())) };
        assert_eq!(&restored[..count as usize], &original);
        Ok(())
    }
}
