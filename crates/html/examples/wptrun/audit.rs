//! Завершённый отчёт связывает каждый GPU readback с его фактической generation.

#[path = "audit/provenance.rs"]
mod provenance;
use gpui::frame_capture::{CapturedFrame, FrameCaptureSink};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{cell::RefCell, io::Read, path::Path, rc::Rc, time::Duration};

#[derive(Clone)]
pub(super) struct Audit(Rc<RefCell<Value>>);

pub(super) fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) fn hash_file(path: &Path) -> std::io::Result<String> {
    hash_reader(&mut std::fs::File::open(path)?)
}

// Аудит не удерживает копию executable или UMD размером с весь файл.
fn hash_reader(reader: &mut impl Read) -> std::io::Result<String> {
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 16 * 1024];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => return Ok(format!("{:x}", digest.finalize())),
            Ok(count) => digest.update(&buffer[..count]),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
}

pub(super) fn fatal(error: impl std::fmt::Display) -> ! {
    eprintln!("INCOMPARABLE_GPU_CONTEXT: {error}");
    std::process::exit(2)
}

impl Audit {
    pub(super) fn new(host: &super::gpu::Host, list: &str, w: f32, h: f32) -> Self {
        let mut value = provenance::manifest(list, w, h).unwrap_or_else(|e| fatal(e));
        value["generations"] = json!({});
        value["requested"] = json!({"selector": host.requested, "strict": true,
        "fallback_reason": null, "resolved": match host.resolved {
            gpui_windows::GpuAdapterPreference::ByLuid { high_part, low_part } =>
                json!({"high_part": high_part, "low_part": low_part}),
            _ => unreachable!("host resolves every policy to a LUID"),
        }});
        let this = Self(Rc::new(RefCell::new(value)));
        this.write().unwrap_or_else(|e| fatal(e));
        this
    }

    pub(super) fn sink(
        &self,
        inner: Option<Box<dyn FrameCaptureSink>>,
    ) -> Box<dyn FrameCaptureSink> {
        Box::new(Endpoint {
            audit: self.clone(),
            inner: inner.unwrap_or_else(|| fatal("GPU readback is required; set WPT_OFFSCREEN=1")),
        })
    }

    pub(super) fn png(&self, path: &Path, shot: &super::capture_dump::Shot) {
        let digest = hash_file(path).unwrap_or_else(|e| fatal(e));
        self.0.borrow_mut()["pngs"].as_array_mut().expect("manifest PNG array").push(json!({
            "path": path.canonicalize().unwrap_or_else(|e| fatal(e)), "sha256": digest, "bgra_sha256": hash(&shot.2),
            "width": shot.0, "height": shot.1,
        }));
    }

    pub(super) fn finish(&self, report: &str) {
        {
            let mut value = self.0.borrow_mut();
            if value["frames"].as_array().is_none_or(Vec::is_empty) {
                fatal("report contains no completed GPU captures");
            }
            value["report_sha256"] =
                json!(hash_file(Path::new(report)).unwrap_or_else(|e| fatal(e)));
            value["driver_dlls"] = provenance::drivers().unwrap_or_else(|e| fatal(e));
            value["complete"] = json!(true);
        }
        self.write().unwrap_or_else(|e| fatal(e));
    }

    fn write(&self) -> std::io::Result<()> {
        let path = provenance::sidecar();
        let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
        std::fs::write(&tmp, serde_json::to_vec_pretty(&*self.0.borrow())?)?;
        std::fs::rename(tmp, path)
    }
}

struct Endpoint {
    audit: Audit,
    inner: Box<dyn FrameCaptureSink>,
}

impl FrameCaptureSink for Endpoint {
    fn completed(&mut self, frame: CapturedFrame, readback: Duration) {
        let mut value = self.audit.0.borrow_mut();
        let context = frame
            .context
            .as_ref()
            .unwrap_or_else(|| fatal("GPU capture has no device context"));
        let generation = context.device.generation.to_string();
        if value["generations"].get(&generation).is_none() {
            value["generations"][&generation] = json!({"device": context.device,
                "driver_dlls": provenance::drivers().unwrap_or_else(|e| fatal(e)),
                "shader_executables": context.shader_executables});
        }
        let frames = value["frames"]
            .as_array_mut()
            .expect("manifest frame array");
        let sequence = frames.len() + 1;
        frames.push(json!({"sequence": sequence, "context": frame.context,
            "width": frame.width, "height": frame.height, "bgra_sha256": hash(&frame.bgra)}));
        drop(value);
        self.inner.completed(frame, readback);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Error, ErrorKind, Result};

    #[test]
    fn streamed_hash_matches_complete_bytes_across_buffer_boundaries() {
        let bytes: Vec<_> = (0..100_000).map(|n| (n % 251) as u8).collect();
        for len in [0, 1, 16_383, 16_384, 16_385, bytes.len()] {
            assert_eq!(
                hash_reader(&mut Cursor::new(&bytes[..len])).unwrap(),
                hash(&bytes[..len])
            );
        }
    }

    #[test]
    fn streamed_hash_retries_interruption_and_preserves_read_errors() {
        struct Faults(bool);
        impl Read for Faults {
            fn read(&mut self, _: &mut [u8]) -> Result<usize> {
                let kind = if std::mem::replace(&mut self.0, false) {
                    ErrorKind::Interrupted
                } else {
                    ErrorKind::PermissionDenied
                };
                Err(Error::from(kind))
            }
        }
        assert_eq!(
            hash_reader(&mut Faults(true)).unwrap_err().kind(),
            ErrorKind::PermissionDenied
        );
    }
}
