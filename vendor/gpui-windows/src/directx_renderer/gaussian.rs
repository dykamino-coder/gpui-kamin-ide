//! KaminIDE patch: separable Gaussian for a group's `filter: blur(σ)` with a small σ.
//!
//! Filter Effects 1 §blur(): the standard deviation of the Gaussian is the given length.
//! The downsampling cascade below clamps its step to 1/5 of a pixel, so every σ under
//! ~2.8 px rendered as the same σ ≈ 1.6 px blur and paid for a 2.5× supersampled
//! first level (6.25× the group's pixels, nine taps each). Like WebRender
//! (`MAX_BLUR_STD_DEVIATION`: downscale only while σ > 4) and Skia (direct kernels for
//! small σ), a σ ≤ 4 device px is convolved exactly at full resolution, straight into the
//! retained group texture: radius ≤ `MAX_SINGLE_PASS_RADIUS` as one 2D pass, larger radii
//! as a horizontal pass into a half-float intermediate plus a vertical pass. Taps are
//! integer texel loads; texels outside the source are transparent black (the filter
//! input outside the group).

use super::*;

/// Largest σ (device px) convolved directly; above it the cascade downsamples.
pub(super) const MAX_DIRECT_SIGMA: f32 = 4.0;
/// Taps per side carried in `BlurQuad::poly` (16 floats).
const MAX_TAPS: usize = 16;
/// Largest kernel radius convolved in one 2D pass ((2r+1)² ≤ 81 loads per pixel). A second
/// pass costs a render-target switch and a read-after-write barrier per group, which on the
/// measured GPU outweighs the extra taps for these radii.
pub(super) const MAX_SINGLE_PASS_RADIUS: usize = 4;

/// Kernel half-width for a direct blur of `sigma`, or `None` when the cascade applies.
pub(super) fn direct_radius(sigma: f32) -> Option<usize> {
    (sigma.is_finite() && sigma > 0.0 && sigma <= MAX_DIRECT_SIGMA)
        .then(|| ((3.0 * sigma).ceil() as usize).clamp(1, MAX_TAPS - 1))
}

/// Margin (device px) the blurred image spreads beyond the group bounds.
pub(super) fn margin(sigma: f32) -> f32 {
    match direct_radius(sigma) {
        Some(r) => r as f32,
        None => (24.0 * (sigma / 14.0).clamp(0.2, 6.0)).min(96.0),
    }
}

/// Normalized one-sided weights w[0..=r] of the sampled Gaussian (w[k] for ±k).
pub(super) fn weights(sigma: f32, r: usize) -> [f32; MAX_TAPS] {
    let mut raw = [0f64; MAX_TAPS];
    let two_var = 2.0 * f64::from(sigma) * f64::from(sigma);
    let mut sum = 0.0;
    for (k, w) in raw.iter_mut().enumerate().take(r + 1) {
        *w = (-((k * k) as f64) / two_var).exp();
        sum += if k == 0 { *w } else { 2.0 * *w };
    }
    let mut out = [0f32; MAX_TAPS];
    for (o, w) in out.iter_mut().zip(raw) {
        *o = (w / sum) as f32;
    }
    out
}

fn create_half_float_target(device: &ID3D11Device, width: u32, height: u32) -> Result<BlurTexture> {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: width,
        Height: height,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_R16G16B16A16_FLOAT,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0) as u32,
        CPUAccessFlags: 0,
        MiscFlags: 0,
    };
    let mut texture: Option<ID3D11Texture2D> = None;
    unsafe { device.CreateTexture2D(&desc, None, Some(&mut texture))? };
    let texture = texture.context("half-float blur target missing")?;
    let (mut srv, mut rtv) = (None, None);
    unsafe {
        device.CreateShaderResourceView(&texture, None, Some(&mut srv))?;
        device.CreateRenderTargetView(&texture, None, Some(&mut rtv))?;
    }
    Ok(BlurTexture {
        width,
        height,
        texture,
        rtv: [rtv],
        srv: [srv],
    })
}

impl DirectXRenderer {
    /// Blur group `index` (raw image in the stage) into its retained texture.
    pub(super) fn gaussian_group(&mut self, index: usize, sigma: f32, r: usize) -> Result<()> {
        let region = self.blur.group_regions[index];
        let (w, h) = (region.width, region.height);
        self.ensure_blur_globals(&self.dev().device.clone())?;
        let taps = weights(sigma, r);
        let poly = [
            [taps[0], taps[1], taps[2], taps[3]],
            [taps[4], taps[5], taps[6], taps[7]],
            [taps[8], taps[9], taps[10], taps[11]],
            [taps[12], taps[13], taps[14], taps[15]],
        ];
        let stage = self
            .blur
            .stage
            .as_ref()
            .context("group stage missing")?
            .srv
            .clone();
        let target = self.blur.groups[index].rtv.clone();
        let frame = [0.0, 0.0, self.width as f32, self.height as f32];
        let origin = [region.x as f32, region.y as f32];
        if r <= MAX_SINGLE_PASS_RADIUS {
            // Whole kernel: frame-domain stage → retained group texture.
            return self.gaussian_pass(
                &stage,
                &target,
                (w, h),
                origin,
                frame,
                6.0,
                [0.0; 2],
                r,
                poly,
            );
        }
        if self
            .blur
            .gauss
            .as_ref()
            .is_none_or(|t| t.width < w || t.height < h)
        {
            // Grow-only, rounded up: group sizes vary by a few pixels between groups.
            let old = self
                .blur
                .gauss
                .as_ref()
                .map_or((0, 0), |t| (t.width, t.height));
            let (gw, gh) = (
                w.max(old.0).next_multiple_of(64),
                h.max(old.1).next_multiple_of(64),
            );
            self.blur.gauss = Some(create_half_float_target(
                &self.dev().device.clone(),
                gw,
                gh,
            )?);
        }
        let gauss = self.blur.gauss.as_ref().unwrap();
        let (gauss_srv, gauss_rtv) = (gauss.srv.clone(), gauss.rtv.clone());
        // Horizontal: frame-domain stage → intermediate (local coordinates).
        self.gaussian_pass(
            &stage,
            &gauss_rtv,
            (w, h),
            origin,
            frame,
            5.0,
            [1.0, 0.0],
            r,
            poly,
        )?;
        // Vertical: intermediate → retained group texture.
        self.gaussian_pass(
            &gauss_srv,
            &target,
            (w, h),
            [0.0, 0.0],
            [0.0, 0.0, w as f32, h as f32],
            5.0,
            [0.0, 1.0],
            r,
            poly,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn gaussian_pass(
        &mut self,
        src: &[Option<ID3D11ShaderResourceView>; 1],
        dst: &[Option<ID3D11RenderTargetView>; 1],
        (w, h): (u32, u32),
        origin: [f32; 2],
        valid: [f32; 4],
        pass: f32,
        direction: [f32; 2],
        r: usize,
        poly: [[f32; 4]; 4],
    ) -> Result<()> {
        let device = self.dev().device.clone();
        let dc = self.dev().device_context.clone();
        let (fw, fh) = (w as f32, h as f32);
        update_buffer(
            &dc,
            self.blur.globals[0].as_ref().unwrap(),
            &[global_params(self.font_info, [fw, fh])],
        )?;
        let quad = BlurQuad {
            bounds: [0.0, 0.0, fw, fh],
            content_mask: [0.0, 0.0, fw, fh],
            src_origin: origin,
            blur_pass: pass,
            pad: r as f32,
            pad2: direction,
            poly,
            source_rect: valid,
            ..Default::default()
        };
        self.pipelines
            .blur_pipeline
            .update_buffer(&device, &dc, &[quad])?;
        let viewport = [D3D11_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: 0.0,
            Width: fw,
            Height: fh,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        }];
        let blend = self.replace_blend()?;
        unsafe { dc.OMSetRenderTargets(Some(dst), None) };
        let pipeline = &self.pipelines.blur_pipeline;
        let (globals, sampler) = (&self.blur.globals, &self.globals.sampler);
        group_stage::downsample_clip(&dc, w, h, || {
            pipeline.draw_with_texture_blended(
                &dc,
                src,
                &viewport,
                globals,
                slice::from_ref(sampler),
                1,
                Some(&blend),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_DIRECT_SIGMA, direct_radius, margin, weights};

    #[test]
    fn small_sigmas_convolve_directly_with_three_sigma_support() {
        assert_eq!(direct_radius(0.6), Some(2));
        assert_eq!(direct_radius(1.0), Some(3));
        assert_eq!(direct_radius(MAX_DIRECT_SIGMA), Some(12));
        assert_eq!(direct_radius(4.01), None);
        assert_eq!(direct_radius(0.0), None);
        assert_eq!(direct_radius(f32::NAN), None);
        assert_eq!(margin(0.6), 2.0);
        // The cascade keeps its own spread for large radii.
        assert_eq!(margin(28.0), 48.0);
    }

    #[test]
    fn weights_are_a_normalized_symmetric_gaussian() {
        for sigma in [0.3f32, 0.6, 1.0, 2.5, 4.0] {
            let r = direct_radius(sigma).unwrap();
            let w = weights(sigma, r);
            let sum: f32 = w[0] + 2.0 * w[1..=r].iter().sum::<f32>();
            assert!((sum - 1.0).abs() < 1e-6, "σ {sigma}: {sum}");
            assert!(w[1..=r].windows(2).all(|p| p[0] > p[1]));
            assert!(w[r + 1..].iter().all(|v| *v == 0.0));
            // Ratio of neighbours is the Gaussian's exp(-(2k+1)/(2σ²)).
            let expect = (-1.0 / (2.0 * sigma * sigma)).exp();
            assert!((w[1] / w[0] - expect).abs() < 1e-5);
        }
    }
}
