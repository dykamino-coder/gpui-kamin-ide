//! KaminIDE patch: group composites whose blend mode the output merger
//! computes, drawn as one series without backdrop copies.
//!
//! A blend mode mixed in the shader reads a copy of the backdrop, and a
//! composite overlapping the previous one must see it already blended: a chain
//! of N overlapping groups (effects-480: every layer overlaps its neighbour)
//! needed N levels, so N copies, N draws and N read-after-write barriers on the
//! frame. For separable modes whose premultiplied formula is a sum of
//! source and destination terms the blender does the mixing itself, as in
//! WebRender (`renderer::BlendMode::from_mix_blend_mode`):
//!
//! - normal: `Cs + Cd·(1 − As)`;
//! - multiply: `Cs·Cd + Cs·(1 − Ad) + Cd·(1 − As) = Cs·(1 − Ad) + Cd·(1 − (As − Cs))`,
//!   dual source (`SV_Target1 = As − Cs`, D3D11 feature level 10+);
//! - screen: `Cs + Cd − Cs·Cd = Cs + Cd·(1 − Cs)`;
//! - exclusion: `Cs + Cd − 2·Cs·Cd = Cs·(1 − Cd) + Cd·(1 − Cs)`;
//! - plus-lighter: `min(1, Cs + Cd)` (Compositing 2 §9.1.15).
//!
//! Alpha is `As + Ad·(1 − As)` for all but plus-lighter (`min(1, As + Ad)`).
//! The output merger blends primitives of one draw in their order (D3D11
//! §17.1), so overlapping composites of a series share a draw: consecutive
//! composites with the same atlas page, mode and mask image are one instanced
//! draw, in document order. Other modes keep the levels of `group_batch`.

use super::*;

/// `GPUI_BLEND_SERIES=0` turns the series off (comparison runs).
pub(super) fn enabled() -> bool {
    std::env::var_os("GPUI_BLEND_SERIES").is_none_or(|v| v != "0")
}

/// Modes the output merger computes (`blend_mode` of `BlurQuad`).
pub(super) fn merger_mode(mode: u32) -> bool {
    matches!(mode, 0 | 1 | 2 | 11 | 16)
}

/// Composite `q` of surface `s` joins a series: a merger mode, and a source
/// the merger receives exactly. The merger takes its inputs at the target's
/// 8-bit precision (D3D11 §17.5 permits it; WARP does), so a source that is
/// not a whole code — the retained image times a fractional opacity, an
/// antialiased rounded or polygon edge, a mask image — would be rounded
/// before the blend and the error would add up along overlapping chains
/// (a 0.35-opacity blurred chain: up to 2 codes off the shader path). The
/// retained image itself holds whole codes, and with coverage 0 or 1 the
/// blender's sum differs from the shader's float formula only on ties.
pub(super) fn joins_series(q: &BlurQuad, s: &PaintSurface, masked: bool) -> bool {
    let r = s.corner_radii;
    let hard_poly = (q.poly_count & 15) < 3 || q.proj[0][3] > 0.5;
    merger_mode(q.blend_mode)
        && q.pad == 1.0
        && !masked
        && hard_poly
        && [r.top_left, r.top_right, r.bottom_right, r.bottom_left]
            .iter()
            .all(|v| v.0 == 0.0)
}

/// Output-merger states of the series modes, made on first use.
#[derive(Default)]
pub(super) struct SeriesBlends {
    states: Vec<(u32, ID3D11BlendState)>,
}

fn series_blend_desc(mode: u32) -> D3D11_BLEND_DESC {
    let mut desc = D3D11_BLEND_DESC::default();
    let rt = &mut desc.RenderTarget[0];
    rt.BlendEnable = true.into();
    rt.BlendOp = D3D11_BLEND_OP_ADD;
    rt.BlendOpAlpha = D3D11_BLEND_OP_ADD;
    rt.SrcBlendAlpha = D3D11_BLEND_ONE;
    rt.DestBlendAlpha = D3D11_BLEND_INV_SRC_ALPHA;
    rt.RenderTargetWriteMask = D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8;
    let (src, dst) = match mode {
        1 => (D3D11_BLEND_INV_DEST_ALPHA, D3D11_BLEND_INV_SRC1_COLOR),
        2 => (D3D11_BLEND_ONE, D3D11_BLEND_INV_SRC_COLOR),
        11 => (D3D11_BLEND_INV_DEST_COLOR, D3D11_BLEND_INV_SRC_COLOR),
        16 => {
            rt.DestBlendAlpha = D3D11_BLEND_ONE;
            (D3D11_BLEND_ONE, D3D11_BLEND_ONE)
        }
        _ => (D3D11_BLEND_ONE, D3D11_BLEND_INV_SRC_ALPHA),
    };
    rt.SrcBlend = src;
    rt.DestBlend = dst;
    desc
}

impl DirectXRenderer {
    fn series_blend(&mut self, mode: u32) -> Result<ID3D11BlendState> {
        if let Some((_, s)) = self.series_blends.states.iter().find(|(m, _)| *m == mode) {
            return Ok(s.clone());
        }
        let desc = series_blend_desc(mode);
        let mut state = None;
        unsafe { self.dev().device.CreateBlendState(&desc, Some(&mut state))? };
        let state = state.context("blend state missing")?;
        self.series_blends.states.push((mode, state.clone()));
        Ok(state)
    }

    /// Draw consecutive composites of output-merger modes in document order.
    pub(super) fn draw_composite_series(&mut self, run: &[&PaintSurface]) -> Result<()> {
        let device = self.dev().device.clone();
        let dc = self.dev().device_context.clone();
        // (instance, atlas page, mask image view); masked composites carry
        // their mask rectangle per instance.
        let mut items = Vec::with_capacity(run.len());
        for s in run {
            let Some((mut quad, page)) = self.group_composite_quad(s) else {
                continue;
            };
            let mask = self
                .blur
                .group_mask
                .get(s.group as usize - 1)
                .cloned()
                .flatten();
            let view = mask.map(|([srv], rect, once)| {
                quad.pad2 = [1.0, once];
                quad.mask_rect = rect;
                srv
            });
            items.push((quad, page, view));
        }
        if items.is_empty() {
            return Ok(());
        }
        self.ensure_blur_globals(&device)?;
        update_buffer(
            &dc,
            self.blur.globals[0].as_ref().unwrap(),
            &[global_params(self.font_info, [self.width as f32, self.height as f32])],
        )?;
        let target = self.current_target().to_vec();
        let mut quads = Vec::new();
        let mut k = 0;
        while k < items.len() {
            let (_, page, view) = &items[k];
            let mode = items[k].0.blend_mode;
            let end = k + items[k..]
                .iter()
                .take_while(|(q, p, v)| p == page && q.blend_mode == mode && v == view)
                .count();
            quads.clear();
            quads.extend(items[k..end].iter().map(|i| i.0));
            let blend = self.series_blend(mode)?;
            self.pipelines
                .group_blend_pipeline
                .update_buffer(&device, &dc, &quads)?;
            if let Some(v) = view {
                unsafe { dc.PSSetShaderResources(3, Some(slice::from_ref(v))) };
            }
            unsafe { dc.OMSetRenderTargets(Some(&target), None) };
            self.pipelines.group_blend_pipeline.draw_with_texture_blended(
                &dc,
                &self.blur.groups[*page].srv,
                slice::from_ref(&self.res().viewport),
                &self.blur.globals,
                slice::from_ref(&self.globals.sampler),
                quads.len() as u32,
                Some(&blend),
            )?;
            k = end;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{merger_mode, series_blend_desc};
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_BLEND_INV_DEST_ALPHA, D3D11_BLEND_INV_SRC_ALPHA, D3D11_BLEND_INV_SRC1_COLOR,
        D3D11_BLEND_ONE,
    };

    #[test]
    fn only_sum_of_terms_modes_go_to_the_merger() {
        let merger: Vec<u32> = (0..=16).filter(|&m| merger_mode(m)).collect();
        assert_eq!(merger, [0, 1, 2, 11, 16]);
    }

    #[test]
    fn multiply_reads_the_second_source_and_alpha_is_source_over() {
        let d = series_blend_desc(1).RenderTarget[0];
        assert_eq!((d.SrcBlend, d.DestBlend), (D3D11_BLEND_INV_DEST_ALPHA, D3D11_BLEND_INV_SRC1_COLOR));
        assert_eq!((d.SrcBlendAlpha, d.DestBlendAlpha), (D3D11_BLEND_ONE, D3D11_BLEND_INV_SRC_ALPHA));
        let p = series_blend_desc(16).RenderTarget[0];
        assert_eq!((p.SrcBlend, p.DestBlend, p.DestBlendAlpha), (D3D11_BLEND_ONE, D3D11_BLEND_ONE, D3D11_BLEND_ONE));
    }
}
