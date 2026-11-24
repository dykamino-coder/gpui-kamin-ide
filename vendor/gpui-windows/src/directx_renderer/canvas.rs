//! KaminIDE patch: the frame is drawn into a kept canvas texture, and the
//! swap chain's back buffer receives a copy of what changed.
//!
//! The canvas always holds the frame drawn last, whatever the swap chain did
//! with its buffers (flip model hands them out in turn, an occluded window
//! drops them). So a scroll step can move the last frame's pixels inside it
//! (`Scene::frame_shift`): two copies through a scratch texture, as a copy
//! within one resource must not overlap, then a raster of the rectangles the
//! move does not explain. GPU and WARP run the same commands; on WARP the move
//! is a memory copy where a raster of the window cost its whole area.
//! Present copies a buffer's accumulated damage from the canvas into it
//! (`Retained::pending`), and frame capture reads the canvas itself.

use super::*;

/// What a frame drew into the canvas, for `--why` (`RasterReport`).
#[derive(Clone, Default)]
pub(super) struct Drawn {
    pub(super) plan: &'static str,
    /// Why a whole frame was drawn (`FullBy`).
    pub(super) full_by: Option<&'static str>,
    /// Why a scroll step was not drawn as a move (`Scene::frame_shift_miss`).
    pub(super) shift_miss: Option<&'static str>,
    pub(super) delta: Option<(i32, i32)>,
    pub(super) moved_px: u64,
    pub(super) rects: Vec<[u32; 4]>,
}

impl Region {
    /// The bounding rectangle of both.
    pub(super) fn union(self, o: Self) -> Self {
        let (x, y) = (self.x.min(o.x), self.y.min(o.y));
        let right = (self.x + self.width).max(o.x + o.width);
        let bottom = (self.y + self.height).max(o.y + o.height);
        Self {
            x,
            y,
            width: right - x,
            height: bottom - y,
        }
    }

    /// From `[x, y, width, height]`, cut to a `w`×`h` target (at least a pixel).
    pub(super) fn of(r: [u32; 4], w: u32, h: u32) -> Self {
        let (x, y) = (r[0].min(w.saturating_sub(1)), r[1].min(h.saturating_sub(1)));
        Self {
            x,
            y,
            width: r[2].min(w - x).max(1),
            height: r[3].min(h - y).max(1),
        }
    }
}

impl DirectXRenderer {
    /// The scratch texture of a move, created once per size.
    fn shift_scratch(&mut self) -> Result<ID3D11Texture2D> {
        let (w, h) = (self.width, self.height);
        if let Some((sw, sh, texture)) = &self.retained.scratch
            && (*sw, *sh) == (w, h)
        {
            return Ok(texture.clone());
        }
        let mut desc = offscreen_target_desc(w, h);
        desc.BindFlags = D3D11_BIND_SHADER_RESOURCE.0 as u32;
        let mut texture = None;
        unsafe {
            self.dev()
                .device
                .CreateTexture2D(&desc, None, Some(&mut texture))?
        };
        let texture = texture.context("shift scratch texture")?;
        self.retained.scratch = Some((w, h, texture.clone()));
        Ok(texture)
    }

    /// Move the canvas by `delta`: its pixels land at `copy` (x, y, w, h).
    pub(super) fn move_canvas(&mut self, delta: (i32, i32), copy: [u32; 4]) -> Result<()> {
        // A frame compared in place keeps its pixels where they are.
        if delta == (0, 0) {
            return Ok(());
        }
        let scratch = self.shift_scratch()?;
        let canvas = self
            .res()
            .render_target
            .clone()
            .context("render target missing")?;
        let ctx = self.dev().device_context.clone();
        let (left, top) = (copy[0] as i32 - delta.0, copy[1] as i32 - delta.1);
        anyhow::ensure!(left >= 0 && top >= 0, "move source outside the canvas");
        let from = D3D11_BOX {
            left: left as u32,
            top: top as u32,
            front: 0,
            right: left as u32 + copy[2],
            bottom: top as u32 + copy[3],
            back: 1,
        };
        let back = D3D11_BOX {
            left: 0,
            top: 0,
            front: 0,
            right: copy[2],
            bottom: copy[3],
            back: 1,
        };
        unsafe {
            ctx.CopySubresourceRegion(&scratch, 0, 0, 0, 0, &canvas, 0, Some(&from));
            ctx.CopySubresourceRegion(&canvas, 0, copy[0], copy[1], 0, &scratch, 0, Some(&back));
        }
        Ok(())
    }

    /// Copy what back buffer `slot` lacks from the canvas into it; the
    /// rectangle copied (x0, y0, x1, y1), the whole target when unknown, or
    /// `None` when it lacks nothing. Every pixel outside it is the frame the
    /// buffer showed, so it bounds what this present changes (dirty rects).
    pub(super) fn copy_to_back_buffer(&mut self) -> Result<Option<[u32; 4]>> {
        let Some(back) = self.res().back_buffer.clone() else {
            return Ok(None);
        };
        let Some(slot) = self.buffer_slot() else {
            return Ok(None);
        };
        let canvas = self
            .res()
            .render_target
            .clone()
            .context("render target missing")?;
        let ctx = self.dev().device_context.clone();
        let (w, h) = (self.width, self.height);
        let rect = match std::mem::replace(&mut self.retained.pending[slot], SceneDamage::Unchanged)
        {
            SceneDamage::Unchanged => return Ok(None),
            SceneDamage::Unknown => [0, 0, w, h],
            SceneDamage::Rect {
                x,
                y,
                width,
                height,
            } => [x.min(w), y.min(h), (x + width).min(w), (y + height).min(h)],
        };
        if rect[0] >= rect[2] || rect[1] >= rect[3] {
            return Ok(None);
        }
        if rect == [0, 0, w, h] {
            unsafe { ctx.CopyResource(&back, &canvas) };
        } else {
            let b = D3D11_BOX {
                left: rect[0],
                top: rect[1],
                front: 0,
                right: rect[2],
                bottom: rect[3],
                back: 1,
            };
            unsafe {
                ctx.CopySubresourceRegion(&back, 0, rect[0], rect[1], 0, &canvas, 0, Some(&b))
            };
        }
        Ok(Some(rect))
    }
}
