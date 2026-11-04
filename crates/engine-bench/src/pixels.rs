//! Точное RGB-сравнение кадров вне замера; сохраняет диагностику без отбрасывания времён.

use super::journal::Journal;
use gpui::frame_capture::CapturedFrame;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub(super) fn save(frame: &CapturedFrame, path: &Path) -> Result<()> {
    let mut rgba = frame.bgra.clone();
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    let mut encoder = png::Encoder::new(std::fs::File::create(path)?, frame.width, frame.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&rgba)?;
    Ok(())
}

fn fingerprint(frame: &CapturedFrame) -> String {
    let hash = frame
        .bgra
        .chunks_exact(4)
        .flat_map(|p| &p[..3])
        .fold(0xcbf29ce484222325u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        });
    format!("{hash:016x}")
}

#[derive(Debug, PartialEq)]
struct Difference {
    pixels: u64,
    bounds: [u32; 4],
}

fn difference(first: &CapturedFrame, frame: &CapturedFrame) -> Option<Difference> {
    let (mut left, mut top, mut right, mut bottom) = (frame.width, frame.height, 0, 0);
    let mut pixels = 0;
    for (index, (a, b)) in first
        .bgra
        .chunks_exact(4)
        .zip(frame.bgra.chunks_exact(4))
        .enumerate()
    {
        if a[..3] != b[..3] {
            let x = (index % frame.width as usize) as u32;
            let y = (index / frame.width as usize) as u32;
            left = left.min(x);
            top = top.min(y);
            right = right.max(x + 1);
            bottom = bottom.max(y + 1);
            pixels += 1;
        }
    }
    (pixels != 0).then(|| Difference {
        pixels,
        bounds: [left, top, right - left, bottom - top],
    })
}

/// Число различающихся RGB-пикселей двух захватов одного размера.
pub(super) fn differing(a: &CapturedFrame, b: &CapturedFrame) -> Result<u64> {
    if (a.width, a.height) != (b.width, b.height) {
        return Err("pixel comparison requires matching capture dimensions".into());
    }
    Ok(difference(a, b).map_or(0, |d| d.pixels))
}

/// Границы расхождения и первые различающиеся пиксели (RGB `a` → `b`).
pub(super) fn detail(a: &CapturedFrame, b: &CapturedFrame) -> Value {
    let first: Vec<Value> = a
        .bgra
        .chunks_exact(4)
        .zip(b.bgra.chunks_exact(4))
        .enumerate()
        .filter(|(_, (p, q))| p[..3] != q[..3])
        .take(8)
        .map(|(i, (p, q))| {
            json!({"x": i % a.width as usize, "y": i / a.width as usize,
                "a": [p[2], p[1], p[0]], "b": [q[2], q[1], q[0]]})
        })
        .collect();
    json!({"bounds": difference(a, b).map(|d| d.bounds), "first": first})
}

pub(super) struct Recorder {
    first: Option<CapturedFrame>,
    first_hash: Option<String>,
    index: usize,
    differences: Journal,
    directory: Option<PathBuf>,
}

impl Recorder {
    pub(super) fn new(directory: Option<&Path>) -> Result<Self> {
        if let Some(path) = directory {
            std::fs::create_dir_all(path)?;
        }
        Ok(Self {
            first: None,
            first_hash: None,
            index: 0,
            differences: Journal::new()?,
            directory: directory.map(Path::to_path_buf),
        })
    }

    pub(super) fn observe(
        &mut self,
        frame: CapturedFrame,
        sequence: u64,
        mode: &str,
        sample: usize,
    ) -> Result<()> {
        self.index += 1;
        let Some(first) = self.first.as_ref() else {
            self.first_hash = Some(fingerprint(&frame));
            if let Some(directory) = &self.directory {
                save(&frame, &directory.join("frame-000001.png"))?;
            }
            self.first = Some(frame);
            return Ok(());
        };
        if (first.width, first.height) != (frame.width, frame.height) {
            return Err("pixel comparison requires matching capture dimensions".into());
        }
        if let Some(diff) = difference(first, &frame) {
            let mut artifacts = Value::Null;
            if self.differences.is_empty()
                && let Some(directory) = &self.directory
            {
                let changed = directory.join(format!("frame-{:06}.png", self.index));
                let path = directory.join(format!("diff-{:06}.png", self.index));
                save(&frame, &changed)?;
                let mut image = CapturedFrame {
                    width: frame.width,
                    height: frame.height,
                    bgra: frame.bgra.clone(),
                    context: frame.context.clone(),
                };
                for (a, b) in image
                    .bgra
                    .chunks_exact_mut(4)
                    .zip(first.bgra.chunks_exact(4))
                {
                    for channel in 0..3 {
                        a[channel] = a[channel].abs_diff(b[channel]);
                    }
                    a[3] = 255;
                }
                save(&image, &path)?;
                artifacts = json!({"baseline": directory.join("frame-000001.png"),
                    "changed": changed, "absolute_rgb_diff": path});
            }
            self.differences
                .push(&json!({"frame_index": self.index, "mode": mode,
                "sample_index": sample, "capture_sequence": sequence, "baseline_frame_index": 1,
                "different_rgb_pixels": diff.pixels, "bounds_xywh": diff.bounds,
                "artifacts": artifacts}))?;
        }
        Ok(())
    }

    pub(super) fn release_baseline(&mut self) {
        self.first = None;
    }

    pub(super) fn changed(&self) -> bool {
        !self.differences.is_empty()
    }

    pub(super) fn report(&mut self) -> Result<Value> {
        let mut report = json!({"first_frame_bgr_fnv1a64": self.first_hash,
            "frame_pixels_changed": !self.differences.is_empty()});
        report["frame_pixel_differences"] = Value::Array(self.differences.read()?);
        Ok(report)
    }
}

#[cfg(test)]
mod tests;
