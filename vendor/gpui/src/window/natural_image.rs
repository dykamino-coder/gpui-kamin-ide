//! Opt-in natural raster painting preserves source phase on the CSS edge grid.

use super::*;

use std::{collections::VecDeque, sync::Weak};

type Key = (usize, usize, u32, u32, u64, u64, u64);
const CAPACITY: usize = 32;

/// Only upload identities are cached. Derived pixels are transient, including
/// for a single 4096-square image; entries never own their source strongly.
#[derive(Default)]
pub(super) struct Cache {
    entries: VecDeque<Entry>,
    journal: Option<Arc<crate::batch_cost::BatchCost>>,
}

struct Entry {
    key: Key,
    source: Weak<RenderImage>,
    image: Arc<RenderImage>,
}

impl Cache {
    pub(super) fn new(instrumentation: &crate::Instrumentation) -> Self {
        Self {
            journal: instrumentation
                .config()
                .gpu_timing
                .then(|| instrumentation.batches.clone()),
            ..Self::default()
        }
    }

    pub(super) fn release_unused(&mut self) {
        let before = self.entries.len();
        self.entries.retain(|entry| entry.source.strong_count() > 0);
        if before != self.entries.len() {
            if let Some(journal) = &self.journal {
                journal.natural_images_dropped((before - self.entries.len()) as u64);
            }
        }
    }

    pub(super) fn heap_report(&self, sink: &mut crate::heap_report::HeapSink) {
        // VecDeque has one allocation, even when it wraps. Its slices expose
        // interior addresses; the ownership reader resolves them to the block.
        let (a, b) = self.entries.as_slices();
        if self.entries.capacity() > 0 {
            let first = if b.is_empty() { a.as_ptr() } else { b.as_ptr() };
            sink(
                "gpui/natural-images",
                first.cast(),
                self.entries.capacity() * std::mem::size_of::<Entry>(),
            );
        }
        for entry in &self.entries {
            entry.image.heap_report(sink, "gpui/natural-images");
        }
    }
}

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
            &mut self.natural_images,
            &image,
            frame,
            w,
            h,
            [phase(bounds.left()), phase(bounds.top())],
            f64::from(sf),
        );
        let (painted, frame) = match data {
            Some(sampled) => (sampled, 0),
            None => (image.clone(), frame),
        };
        let result = self.paint_image_with_sampling_tile(
            bounds,
            corners,
            painted,
            frame,
            grayscale,
            ImageSampling::NearestSnapped,
        );
        // The lazy upload reads this source synchronously in the atlas callback.
        // Retained scenes keep the uploaded tile, not a CPU source or pixel copy.
        drop(image);
        result?;
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn resample(
    cache: &mut Cache,
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
    if let Some(index) = cache.entries.iter().position(|entry| entry.key == key) {
        let entry = cache.entries.remove(index)?;
        let result = entry.image.clone();
        cache.entries.push_back(entry);
        return Some(result);
    }
    let source = Arc::downgrade(image);
    let journal = cache.journal.clone();
    let result = Arc::new(RenderImage::from_lazy_bgra(
        w,
        h,
        Arc::new(Resample {
            source: source.clone(),
            frame,
            w,
            h,
            iw,
            ih,
            phase,
            scale,
            journal: journal.clone(),
        }),
    )?);
    cache.release_unused();
    if cache.entries.len() == CAPACITY {
        cache.entries.pop_front();
        if let Some(journal) = &journal {
            journal.natural_images_dropped(1);
        }
    }
    cache.entries.push_back(Entry {
        key,
        source,
        image: result.clone(),
    });
    if let Some(journal) = journal {
        journal.natural_image_created();
    }
    Some(result)
}

struct Resample {
    source: Weak<RenderImage>,
    frame: usize,
    w: u32,
    h: u32,
    iw: u32,
    ih: u32,
    phase: [f64; 2],
    scale: f64,
    journal: Option<Arc<crate::batch_cost::BatchCost>>,
}

impl crate::LazyBgra for Resample {
    fn bgra(&self) -> Option<Vec<u8>> {
        let source = self.source.upgrade()?;
        let bytes = source.upload_bytes(self.frame)?;
        let mut out = Vec::with_capacity((self.w * self.h * 4) as usize);
        for y in 0..self.h {
            let sy = sample(y, self.ih, self.phase[1], self.scale);
            for x in 0..self.w {
                let sx = sample(x, self.iw, self.phase[0], self.scale);
                let at = ((sy * self.iw + sx) * 4) as usize;
                out.extend_from_slice(&bytes[at..at + 4]);
            }
        }
        if let Some(journal) = &self.journal {
            journal.natural_image_rasterized(out.len() as u64);
        }
        Some(out)
    }
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

#[cfg(test)]
mod tests;
