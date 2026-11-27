//! Keep one bounded journal snapshot even when the UI thread cannot finish a frame.

use serde_json::json;
use std::{
    path::Path,
    sync::mpsc::{self, Sender},
    thread::JoinHandle,
    time::{Duration, Instant},
};

/// A diagnostic reader whose observer is joined on finish or drop.
/// Call `finish` to receive I/O errors; dropping only stops and joins the worker.
pub struct Writer {
    stop: Sender<()>,
    worker: Option<JoinHandle<Result<(), String>>>,
    _reader: crate::why::Subscription,
}

/// A directory enables one reader and one observer thread; `None` does nothing.
/// The worker observes the same engine journal consumed by the frame report.
pub fn start(directory: Option<&Path>) -> Result<Option<Writer>, Box<dyn std::error::Error>> {
    let Some(directory) = directory else {
        return Ok(None);
    };
    std::fs::create_dir_all(directory)?;
    let path = directory.join("progress.json");
    let reader = crate::why::subscribe();
    let started = Instant::now();
    write(&path, started, false)?;
    let (stop, receiver) = mpsc::channel();
    let worker = std::thread::Builder::new()
        .name("why-progress".into())
        .spawn(move || {
            loop {
                let finished = !matches!(
                    receiver.recv_timeout(Duration::from_secs(1)),
                    Err(mpsc::RecvTimeoutError::Timeout)
                );
                write(&path, started, finished).map_err(|error| error.to_string())?;
                if finished {
                    return Ok(());
                }
            }
        })?;
    Ok(Some(Writer {
        stop,
        worker: Some(worker),
        _reader: reader,
    }))
}

fn write(path: &Path, started: Instant, finished: bool) -> Result<(), Box<dyn std::error::Error>> {
    let value = json!({
        "elapsed_ms": started.elapsed().as_secs_f64() * 1000.,
        "finished": finished,
        "scope": "non-consuming engine journal; timings since the last frame take; intervals overlap",
        "journal": crate::why::snapshot(),
    });
    // One file is replaced, never appended: a stalled process cannot grow a log forever.
    let pending = path.with_extension("json.tmp");
    std::fs::write(&pending, serde_json::to_vec_pretty(&value)?)?;
    std::fs::rename(pending, path)?;
    Ok(())
}

impl Writer {
    /// Stop observing, save a final snapshot and report any observer failure.
    pub fn finish(mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.join().map_err(Into::into)
    }

    fn join(&mut self) -> Result<(), String> {
        let Some(worker) = self.worker.take() else {
            return Ok(());
        };
        let _ = self.stop.send(());
        worker.join().map_err(|_| "journal observer panicked")?
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        let _ = self.join();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_progress_creates_no_worker() {
        assert!(start(None).unwrap().is_none());
    }

    #[test]
    fn finishing_joins_the_observer_and_keeps_the_last_complete_snapshot() {
        let (_one, _reader) = crate::why::test_reader();
        let directory = std::env::temp_dir().join(format!("why-progress-{}", std::process::id()));
        let writer = start(Some(&directory)).unwrap().unwrap();
        drop(crate::why::Span::new("test.observed_pass"));
        writer.finish().unwrap();
        let path = directory.join("progress.json");
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(value["finished"], true);
        assert_eq!(
            value["journal"]["timings_ms"]["test.observed_pass"]["calls"],
            1
        );
        assert!(!path.with_extension("json.tmp").exists());
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}
