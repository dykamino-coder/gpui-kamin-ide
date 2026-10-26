//! Точное RGB-сравнение кадров вне замера; сохраняет диагностику без отбрасывания времён.

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

pub(super) struct Recorder {
    first: Option<CapturedFrame>,
    first_hash: Option<String>,
    index: usize,
    differences: Vec<Value>,
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
            differences: Vec::new(),
            directory: directory.map(Path::to_path_buf),
        })
    }

    pub(super) fn observe(
        &mut self,
        frame: CapturedFrame,
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
                    seq: frame.seq,
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
                .push(json!({"frame_index": self.index, "mode": mode,
                "sample_index": sample, "capture_sequence": frame.seq, "baseline_frame_index": 1,
                "different_rgb_pixels": diff.pixels, "bounds_xywh": diff.bounds,
                "artifacts": artifacts}));
        }
        Ok(())
    }

    pub(super) fn report(&self) -> Value {
        json!({"first_frame_bgr_fnv1a64": self.first_hash,
            "frame_pixels_changed": !self.differences.is_empty(),
            "frame_pixel_differences": self.differences})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(bgra: Vec<u8>) -> CapturedFrame {
        CapturedFrame {
            width: 2,
            height: 2,
            bgra,
            seq: 7,
        }
    }

    #[test]
    fn exact_rgb_comparison_ignores_alpha_and_bounds_all_changed_pixels() {
        let first = frame(vec![0; 16]);
        let mut changed = frame(vec![0; 16]);
        changed.bgra[3] = 255;
        assert_eq!(difference(&first, &changed), None);
        changed.bgra[4] = 1;
        changed.bgra[10] = 2;
        assert_eq!(
            difference(&first, &changed),
            Some(Difference {
                pixels: 2,
                bounds: [0, 0, 2, 2]
            })
        );
    }

    #[test]
    fn changing_frames_are_recorded_without_rejection_or_replacing_baseline() {
        let first = frame(vec![0; 16]);
        let expected_hash = fingerprint(&first);
        let mut recorder = Recorder::new(None).expect("no IO");
        recorder.observe(first, "cold", 1).expect("baseline");
        let mut changed = frame(vec![0; 16]);
        changed.bgra[0] = 1;
        recorder
            .observe(changed, "cold", 2)
            .expect("changed frame accepted");
        recorder
            .observe(frame(vec![0; 16]), "warm", 1)
            .expect("baseline restored");
        let report = recorder.report();
        assert_eq!(report["first_frame_bgr_fnv1a64"], expected_hash);
        assert_eq!(report["frame_pixels_changed"], true);
        assert_eq!(
            report["frame_pixel_differences"].as_array().map(Vec::len),
            Some(1)
        );
        assert_eq!(report["frame_pixel_differences"][0]["frame_index"], 2);
        assert_eq!(report["frame_pixel_differences"][0]["sample_index"], 2);
    }
}
