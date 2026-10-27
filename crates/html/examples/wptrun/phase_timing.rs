//! Измерения фаз стенда в stderr: включаются WPT_PHASE_TIMING, отчёт пар не меняют.

use kamin_html::host::capture::CaptureSession;
use std::time::{Duration, Instant};

pub(super) fn enabled() -> bool {
    std::env::var_os("WPT_PHASE_TIMING").is_some()
}

pub(super) fn emit(source: Option<&str>, phase: &str, elapsed: Duration, steps: u32) {
    if enabled() {
        eprintln!(
            "WPT_PHASE|{}|{phase}|{}|{steps}",
            source.unwrap_or("<separator>"),
            elapsed.as_micros()
        );
    }
}

pub(super) struct Phase<'a> {
    source: Option<&'a str>,
    phase: &'a str,
    started: Instant,
}

impl<'a> Phase<'a> {
    pub(super) fn new(source: Option<&'a str>, phase: &'a str) -> Self {
        Self {
            source,
            phase,
            started: Instant::now(),
        }
    }
}

impl Drop for Phase<'_> {
    fn drop(&mut self) {
        emit(self.source, self.phase, self.started.elapsed(), 0);
    }
}

pub(super) struct PageTiming<'a> {
    pub(super) started: Instant,
    pub(super) steps: u32,
    pub(super) readback: Duration,
    pub(super) first: Option<Duration>,
    source: Option<&'a str>,
    gpu_readback_start: Duration,
    capture: Option<CaptureSession>,
}

impl<'a> PageTiming<'a> {
    pub(super) fn new(source: Option<&'a str>, capture: Option<CaptureSession>) -> Self {
        Self {
            source,
            started: Instant::now(),
            steps: 0,
            readback: Duration::ZERO,
            first: None,
            gpu_readback_start: capture
                .as_ref()
                .map_or(Duration::ZERO, CaptureSession::readback_time),
            capture,
        }
    }
}

impl Drop for PageTiming<'_> {
    fn drop(&mut self) {
        let total = self.started.elapsed();
        emit(self.source, "frame-total", total, self.steps);
        emit(
            self.source,
            "settle",
            total.saturating_sub(self.first.unwrap_or_default()),
            self.steps,
        );
        emit(
            self.source,
            "readback",
            self.capture
                .as_ref()
                .map_or(Duration::ZERO, CaptureSession::readback_time)
                .saturating_sub(self.gpu_readback_start),
            self.steps,
        );
        emit(self.source, "readback-consume", self.readback, self.steps);
        if let Some(first) = self.first {
            emit(self.source, "first-frame", first, 1);
        }
    }
}
