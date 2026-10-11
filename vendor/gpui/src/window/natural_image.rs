//! Opt-in natural raster painting preserves source phase on the CSS edge grid.

use super::*;
use std::sync::{Mutex, OnceLock};

type Cache = std::collections::HashMap<(usize, usize, u32, u32, u64, u64, u64), Arc<RenderImage>>;
static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();

impl Window {
    /// Paint an unresized CSS raster with device-snapped destination edges and
    /// nearest sampling in its original, unrounded source coordinate system.
    pub fn paint_natural_image(
        &mut self,
        bounds: Bounds<Pixels>,
        corners: Corners<Pixels>,
        image: Arc<RenderImage>,
        frame: usize,
        grayscale: bool,
    ) -> Result<()> {
        if self.current_transformation() != TransformationMatrix::unit() {
            return self.paint_image_with_sampling(
                bounds,
                corners,
                image,
                frame,
                grayscale,
                ImageSampling::Nearest,
            );
        }
        let sf = self.scale_factor();
        let physical = bounds.scale(sf);
        let w = (physical.right().round() - physical.left().round())
            .0
            .max(0.0) as u32;
        let h = (physical.bottom().round() - physical.top().round())
            .0
            .max(0.0) as u32;
        let phase = |v: Pixels| {
            let device = f64::from(f32::from(v)) * f64::from(sf);
            device.round() - device
        };
        let data = resample(
            &image,
            frame,
            w,
            h,
            [phase(bounds.left()), phase(bounds.top())],
            f64::from(sf),
        );
        let (image, frame) = match data {
            Some(image) => (image, 0),
            None => (image, frame),
        };
        self.paint_image_with_sampling(
            bounds,
            corners,
            image,
            frame,
            grayscale,
            ImageSampling::NearestSnapped,
        )
    }
}

fn resample(
    image: &Arc<RenderImage>,
    frame: usize,
    w: u32,
    h: u32,
    phase: [f64; 2],
    scale: f64,
) -> Option<Arc<RenderImage>> {
    let source = image.size(frame);
    let (iw, ih) = (
        u32::try_from(source.width.0).ok()?,
        u32::try_from(source.height.0).ok()?,
    );
    if iw == 0
        || ih == 0
        || w == 0
        || h == 0
        || w > 4096
        || h > 4096
        || scale <= 0.0
        || (scale == 1.0 && phase == [0.0; 2])
    {
        return None;
    }
    let key = (
        image.id.0,
        frame,
        w,
        h,
        phase[0].to_bits(),
        phase[1].to_bits(),
        scale.to_bits(),
    );
    let cache = CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    if let Ok(cache) = cache.lock()
        && let Some(found) = cache.get(&key)
    {
        return Some(found.clone());
    }
    let bytes = image.as_bytes(frame)?;
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        let sy = sample(y, ih, phase[1], scale);
        for x in 0..w {
            let sx = sample(x, iw, phase[0], scale);
            let at = ((sy * iw + sx) * 4) as usize;
            out.extend_from_slice(&bytes[at..at + 4]);
        }
    }
    let result = crate::bgra_bytes_to_image(w, h, out)?;
    if let Ok(mut cache) = cache.lock() {
        if cache.len() >= 32 {
            cache.clear();
        }
        cache.insert(key, result.clone());
    }
    Some(result)
}

fn sample(pixel: u32, source: u32, phase: f64, scale: f64) -> u32 {
    // CSS Images 3 §5.2 permits nearest sampling. Choosing the preceding
    // texel on an exact boundary rounds the painted edge upward, like CSS
    // boxes. Keep source phase after snapping destination edges (Blink
    // background_image_geometry.cc:114-122), so fractional placement never
    // stretches or independently shifts the internal texel grid.
    (((f64::from(pixel) + 0.5 + phase) / scale).ceil() - 1.0).clamp(0.0, f64::from(source - 1))
        as u32
}
